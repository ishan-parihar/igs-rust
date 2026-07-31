use crate::cache::FeedCache;
use crate::types::{FeedCacheEntry, HttpSettings, NewsItem, Settings};
use crate::{AppError, AppResult};
use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use url::Url;

/// Validate that a URL is safe to fetch (SSRF protection).
/// Rejects non-http(s) schemes, loopback, link-local, private, and reserved IPs.
pub fn validate_public_url(url_str: &str) -> AppResult<Url> {
    let url = Url::parse(url_str)
        .map_err(|e| AppError::validation(format!("invalid URL: {e}")))?;
    
    // Only allow http/https schemes
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::validation(
            "only http and https schemes are allowed".to_string()));
    }
    
    // Get host and resolve to IP
    let host = url.host_str().ok_or_else(|| 
        AppError::validation("URL missing host".to_string()))?;
    
    // Try to parse as IP address first
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_or_reserved_ip(&ip) {
            return Err(AppError::validation(
                "access to private/reserved IP addresses is not allowed".to_string()));
        }
        // IP is public, OK
        return Ok(url);
    }
    
    // Host is a domain name - resolve it
    // We'll do a basic check for obvious internal domains
    if is_obviously_internal_domain(host) {
        return Err(AppError::validation(
            "access to internal domains is not allowed".to_string()));
    }
    
    // For full DNS resolution, we'd need a DNS resolver.
    // For now, allow the request but the HTTP client will resolve at connect time.
    // A production deployment should add a DNS resolution step here.
    Ok(url)
}

/// Check if an IP address is private, loopback, link-local, or reserved.
fn is_private_or_reserved_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_unspecified()
                || matches!(ipv4.octets(), [127, ..])
                || matches!(ipv4.octets(), [169, 254, ..])
                || matches!(ipv4.octets(), [10, ..])
                || matches!(ipv4.octets(), [172, 16..=31, ..])
                || matches!(ipv4.octets(), [192, 168, ..])
                || matches!(ipv4.octets(), [100, 64..=127, ..])
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.segments()[0] & 0xfe00 == 0xfc00
                || ipv6.segments()[0] & 0xffc0 == 0xfe80
                || ipv6.is_unique_local()
        }
    }
}

/// Quick check for obviously internal domains (not exhaustive).
fn is_obviously_internal_domain(host: &str) -> bool {
    let host = host.to_lowercase();
    host == "localhost"
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".corp")
        || host.ends_with(".lan")
        || host.ends_with(".home")
        || host == "metadata"
        || host == "metadata.google.internal"
        || host == "169.254.169.254" // AWS metadata
        || host == "metadata.azure.com" // Azure metadata
        || host == "metadata.google.internal" // GCP metadata
}

/// HTTP fetch result
pub struct FetchResponse {
    pub status: u16,
    pub headers: reqwest::header::HeaderMap,
    pub body_text: String,
}

pub enum FetchOutcome {
    Cached(FeedCacheEntry),
    Response(FetchResponse, Option<String>, Option<String>), // response, etag, last-modified
}

/// Per-host concurrency tracker
struct HostSemaphoreMap {
    default_per_host: u32,
    map: Mutex<HashMap<String, Arc<Semaphore>>>,
}

impl HostSemaphoreMap {
    fn new(default_per_host: u32) -> Self {
        Self {
            default_per_host,
            map: Mutex::new(HashMap::new()),
        }
    }

    async fn acquire(&self, host: &str) -> tokio::sync::OwnedSemaphorePermit {
        let sem = {
            let mut map = self.map.lock().await;
            map.entry(host.to_string())
                .or_insert_with(|| Arc::new(Semaphore::new(self.default_per_host as usize)))
                .clone()
        };
        sem.acquire_owned().await.expect("semaphore closed")
    }
}

/// HTTP client with caching, retries, per-host concurrency, and exponential backoff
pub struct HttpClient {
    client: Client,
    cache: FeedCache,
    settings: HttpSettings,
    global_semaphore: Semaphore,
    host_semaphores: HostSemaphoreMap,
}

impl HttpClient {
    pub fn new(settings: &HttpSettings, cache_dir: &Path) -> AppResult<Self> {
        let timeout = Duration::from_millis(settings.timeout_ms);
        let client = Client::builder()
            .user_agent(&settings.user_agent)
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()
            .map_err(|e| AppError::Http(e))?;

        Ok(Self {
            cache: FeedCache::new(cache_dir),
            client,
            settings: settings.clone(),
            global_semaphore: Semaphore::new(settings.concurrency as usize),
            host_semaphores: HostSemaphoreMap::new(settings.per_host),
        })
    }

    /// Extract host from URL for per-host concurrency
    fn extract_host(url: &str) -> String {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub async fn fetch(
        &self,
        url: &str,
        extra_headers: Option<&HashMap<String, String>>,
        cache_mode: &str,
    ) -> Result<FetchOutcome> {
        let cached = self.cache.read(url).await.ok().flatten();
        // SSRF protection: validate URL before fetching
        let _ = crate::http::validate_public_url(url)?;

        // If cache-only mode, return cached if available
        if cache_mode == "only" {
            if let Some(entry) = cached {
                return Ok(FetchOutcome::Cached(entry));
            }
            return Err(anyhow::anyhow!("Cache miss for {}", url));
        }

        // If prefer mode and cache is valid, return cached
        if cache_mode == "prefer" {
            if let Some(ref entry) = cached {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                if now - entry.fetched_at <= 1_800_000 {
                    return Ok(FetchOutcome::Cached(entry.clone()));
                }
            }
        }

        // Acquire both global and per-host semaphore
        let _global_permit = self
            .global_semaphore
            .acquire()
            .await
            .map_err(|e| anyhow::anyhow!("Global semaphore closed: {}", e))?;
        let host = Self::extract_host(url);
        let _host_permit = self.host_semaphores.acquire(&host).await;

        // Retry loop with exponential backoff
        let mut last_err: Option<anyhow::Error> = None;
        let max_retries = self.settings.retries;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let backoff_ms = self.settings.backoff_base_ms as f64
                    * self.settings.backoff_factor.powi(attempt as i32 - 1);
                tokio::time::sleep(Duration::from_millis(backoff_ms as u64)).await;
            }

            let result = self
                .execute_request(url, extra_headers, cached.as_ref())
                .await;

            match result {
                Ok(outcome) => {
                    // 4xx responses are returned as Ok(FetchOutcome::Response) by
                    // execute_request; only 5xx responses become Err. The previous
                    // string-match heuristic (`err_str.contains("status") && err_str.contains("4")`)
                    // was both dead code (4xx never reaches this branch) and buggy
                    // (it false-matched URLs containing "4" like "/v4/"). 5xx errors
                    // should always retry, so we just record and continue.
                    return Ok(outcome);
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err
            .unwrap_or_else(|| anyhow::anyhow!("Request failed after {} retries", max_retries)))
    }

    /// Execute a single HTTP request attempt
    async fn execute_request(
        &self,
        url: &str,
        extra_headers: Option<&HashMap<String, String>>,
        cached: Option<&FeedCacheEntry>,
    ) -> Result<FetchOutcome> {
        let mut req = self.client.get(url);

        // Conditional request headers
        if let Some(entry) = cached {
            if let Some(ref etag) = entry.etag {
                req = req.header("if-none-match", etag);
            }
            if let Some(ref lm) = entry.last_modified {
                req = req.header("if-modified-since", lm);
            }
        }

        if let Some(h) = extra_headers {
            for (k, v) in h {
                req = req.header(k.as_str(), v.as_str());
            }
        }

        let res = req.send().await?;
        let status = res.status().as_u16();
        let headers = res.headers().clone();
        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let last_modified = headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if status == 304 {
            if let Some(entry) = cached {
                return Ok(FetchOutcome::Cached(entry.clone()));
            }
            return Ok(FetchOutcome::Cached(FeedCacheEntry {
                url: url.to_string(),
                etag,
                last_modified,
                fetched_at: 0,
                items: vec![],
            }));
        }

        // Treat 5xx as errors for retry purposes
        if status >= 500 {
            return Err(anyhow::anyhow!("Server error HTTP {} for {}", status, url));
        }

        let body_text = res.text().await?;

        Ok(FetchOutcome::Response(
            FetchResponse {
                status,
                headers,
                body_text,
            },
            etag,
            last_modified,
        ))
    }

    /// POST JSON body to a URL. No caching — used for API calls (Tavily, Firecrawl, etc.)
    pub async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        extra_headers: Option<&HashMap<String, String>>,
    ) -> Result<FetchOutcome> {
        // SSRF protection: validate URL before posting
        let _ = crate::http::validate_public_url(url)?;
        let _global_permit = self.global_semaphore
            .acquire()
            .await
            .map_err(|e| anyhow::anyhow!("Global semaphore closed: {}", e))?;
        let host = Self::extract_host(url);
        let _host_permit = self.host_semaphores.acquire(&host).await;

        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(body);

        if let Some(h) = extra_headers {
            for (k, v) in h {
                req = req.header(k.as_str(), v.as_str());
            }
        }

        let res = req.send().await?;
        let status = res.status().as_u16();
        let headers = res.headers().clone();
        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let last_modified = headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if status >= 500 {
            return Err(anyhow::anyhow!("Server error HTTP {} for {}", status, url));
        }

        let body_text = res.text().await?;

        Ok(FetchOutcome::Response(
            FetchResponse {
                status,
                headers,
                body_text,
            },
            etag,
            last_modified,
        ))
    }

    pub async fn write_cache(
        &self,
        url: &str,
        items: Vec<NewsItem>,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<()> {
        self.cache.write(url, etag, last_modified, items).await
    }
}

/// Resolve cache directory: absolute path as-is, relative paths resolved against user config dir
pub fn resolve_cache_dir(settings: &Settings, user_cfg_dir: &Path) -> PathBuf {
    let cache_path = PathBuf::from(&settings.cache.dir);
    if cache_path.is_absolute() {
        cache_path
    } else {
        user_cfg_dir.join(&cache_path)
    }
}
