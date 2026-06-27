use crate::cache::FeedCache;
use crate::types::{FeedCacheEntry, HttpSettings, NewsItem, Settings};
use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};

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
    pub fn new(settings: &HttpSettings, cache_dir: &Path) -> Self {
        let timeout = Duration::from_millis(settings.timeout_ms);
        let client = Client::builder()
            .user_agent(&settings.user_agent)
            .timeout(timeout)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            cache: FeedCache::new(cache_dir),
            client,
            settings: settings.clone(),
            global_semaphore: Semaphore::new(settings.concurrency as usize),
            host_semaphores: HostSemaphoreMap::new(settings.per_host),
        }
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
                    // Cache successful responses
                    if let FetchOutcome::Response(ref resp, ref _etag, ref _lm) = outcome {
                        if resp.status >= 200 && resp.status < 400 {
                            // Don't cache here — let the caller decide what to cache
                        }
                    }
                    return Ok(outcome);
                }
                Err(e) => {
                    last_err = Some(e);
                    // Don't retry on client errors (4xx) — only server errors and network failures
                    if let Some(ref err) = last_err {
                        let err_str = err.to_string().to_lowercase();
                        // If it's a reqwest error that's not a server error, don't retry
                        if err_str.contains("status") && err_str.contains("4") {
                            break;
                        }
                    }
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
        let _global_permit = self
            .global_semaphore
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

    pub fn cache(&self) -> &FeedCache {
        &self.cache
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
