use crate::types::{FeedCacheEntry, NewsItem, QueryCacheEntry, QueryCacheMeta};
use anyhow::Result;
use base64::Engine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Base64 engine for cache key encoding
fn b64_encode(data: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE;
    URL_SAFE.encode(data)
}

/// Directory-based feed cache storing parsed items keyed by URL.
pub struct FeedCache {
    dir: PathBuf,
}

impl FeedCache {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    fn file_for(&self, url: &str) -> PathBuf {
        let key = b64_encode(url.as_bytes());
        self.dir.join(format!("{}.json", key))
    }

    pub async fn read(&self, url: &str) -> Result<Option<FeedCacheEntry>> {
        let file = self.file_for(url);
        match fs::read_to_string(&file).await {
            Ok(raw) => {
                let entry: FeedCacheEntry = serde_json::from_str(&raw)?;
                Ok(Some(entry))
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn write(
        &self,
        url: &str,
        etag: Option<String>,
        last_modified: Option<String>,
        items: Vec<NewsItem>,
    ) -> Result<()> {
        let file = self.file_for(url);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).await?;
        }
        let entry = FeedCacheEntry {
            url: url.to_string(),
            etag,
            last_modified,
            fetched_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            items,
        };
        let raw = serde_json::to_string(&entry)?;
        fs::write(&file, raw.as_bytes()).await?;
        Ok(())
    }
}

/// Query-level cache: caches aggregated query results keyed by a compound string.
pub struct QueryCache {
    dir: PathBuf,
    ttl_ms: u64,
}

impl QueryCache {
    pub fn new(dir: &Path, ttl_ms: u64) -> Self {
        Self {
            dir: dir.join("queries"),
            ttl_ms,
        }
    }

    fn file_for(&self, key: &str) -> PathBuf {
        let k = b64_encode(key.as_bytes());
        self.dir.join(format!("{}.json", k))
    }

    pub async fn read<T: serde::de::DeserializeOwned + Clone>(
        &self,
        key: &str,
    ) -> Result<Option<(QueryCacheMeta, T)>> {
        let file = self.file_for(key);
        match fs::read_to_string(&file).await {
            Ok(raw) => {
                let entry: QueryCacheEntry<T> = serde_json::from_str(&raw)?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                if now - entry.meta.at > self.ttl_ms {
                    return Ok(None);
                }
                Ok(Some((entry.meta, entry.data)))
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn write<T: serde::Serialize>(
        &self,
        key: &str,
        deps: HashMap<String, u64>,
        data: &T,
    ) -> Result<()> {
        let file = self.file_for(key);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).await?;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let entry = QueryCacheEntry {
            meta: QueryCacheMeta {
                key: key.to_string(),
                at: now,
                deps,
            },
            data,
        };
        let raw = serde_json::to_string(&entry)?;
        fs::write(&file, raw.as_bytes()).await?;
        Ok(())
    }
}
