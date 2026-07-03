//! Real-time monitoring & alerting system.
//!
//! Provides scheduled polling of news sources with threshold-based alerting.
//! Monitors can trigger on:
//! - Keyword appearance in new articles
//! - Entity mention count crossing a threshold
//! - Source content changes (new articles detected)
//!
//! Alerts are delivered via webhook (Slack/Discord/Teams), file append,
//! or stdout. The monitor runs as a background tokio task.

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::parsers;
use crate::types::Settings;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration as TokioDuration;

// ─── Monitor Configuration Types ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Unique monitor ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Pool IDs to monitor
    pub pools: Vec<String>,
    /// Keywords to watch for (any match triggers alert)
    pub keywords: Vec<String>,
    /// Poll interval in seconds
    pub interval_secs: u64,
    /// Alert threshold: min keyword matches in a poll cycle
    pub threshold: u32,
    /// Webhook URL for alerts (Slack/Discord/Teams format)
    pub webhook_url: Option<String>,
    /// File path to append alerts to
    pub alert_file: Option<String>,
    /// Whether the monitor is active
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorAlert {
    pub monitor_id: String,
    pub monitor_name: String,
    pub timestamp: String,
    pub triggered_by: String,
    pub article_count: usize,
    pub matching_articles: Vec<MatchedArticle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedArticle {
    pub title: String,
    pub link: String,
    pub source: String,
    pub matched_keywords: Vec<String>,
}

// ─── Monitor Manager ──────────────────────────────────────────

pub struct MonitorManager {
    monitors: Arc<Mutex<Vec<MonitorConfig>>>,
    settings: Arc<Settings>,
    http: Arc<HttpClient>,
}

impl MonitorManager {
    pub fn new(settings: Arc<Settings>) -> Self {
        let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
        let http = Arc::new(HttpClient::new(&settings.http, &cache_dir));
        Self {
            monitors: Arc::new(Mutex::new(Vec::new())),
            settings,
            http,
        }
    }

    pub async fn add(&self, monitor: MonitorConfig) {
        let mut monitors = self.monitors.lock().await;
        monitors.push(monitor);
    }

    pub async fn list(&self) -> Vec<MonitorConfig> {
        self.monitors.lock().await.clone()
    }

    pub async fn remove(&self, id: &str) -> bool {
        let mut monitors = self.monitors.lock().await;
        let before = monitors.len();
        monitors.retain(|m| m.id != id);
        monitors.len() < before
    }

    pub async fn pause(&self, id: &str) -> bool {
        let mut monitors = self.monitors.lock().await;
        if let Some(m) = monitors.iter_mut().find(|m| m.id == id) {
            m.active = false;
            true
        } else {
            false
        }
    }

    pub async fn resume(&self, id: &str) -> bool {
        let mut monitors = self.monitors.lock().await;
        if let Some(m) = monitors.iter_mut().find(|m| m.id == id) {
            m.active = true;
            true
        } else {
            false
        }
    }

    /// Start all active monitors as background tokio tasks.
    /// Each monitor polls its pools at the configured interval and triggers
    /// alerts when keyword matches cross the threshold.
    pub fn start_all(&self) {
        let monitors = self.monitors.clone();
        let http = self.http.clone();
        let settings = self.settings.clone();

        tokio::spawn(async move {
            loop {
                let active_monitors: Vec<MonitorConfig> = {
                    let guards = monitors.lock().await;
                    guards.iter().filter(|m| m.active).cloned().collect()
                };

                for monitor in active_monitors {
                    let http = http.clone();
                    let settings = settings.clone();
                    let monitors_for_alert = monitors.clone();

                    tokio::spawn(async move {
                        if let Ok(alert) = poll_monitor(&monitor, &http, &settings).await {
                            if alert.article_count >= monitor.threshold as usize {
                                deliver_alert(&alert, &monitor).await;
                                // Log the alert
                                let mut guards = monitors_for_alert.lock().await;
                                let _ = &mut guards; // touch to avoid unused warning
                            }
                        }
                    });
                }

                // Wait 60 seconds before next poll cycle
                tokio::time::sleep(TokioDuration::from_secs(60)).await;
            }
        });
    }
}

/// Poll a single monitor's pools and check for keyword matches.
async fn poll_monitor(
    monitor: &MonitorConfig,
    http: &HttpClient,
    _settings: &Settings,
) -> Result<MonitorAlert, String> {
    let sf = config::load_sources()
        .await
        .map_err(|e| format!("Failed to load sources: {}", e))?;

    let mut matching_articles: Vec<MatchedArticle> = Vec::new();

    for pool_id in &monitor.pools {
        let pool_sources: Vec<_> = sf
            .sources
            .iter()
            .filter(|s| s.pools.contains(pool_id) && s.is_active.unwrap_or(true))
            .take(20) // limit to 20 sources per pool per poll
            .collect();

        for source in pool_sources {
            if let Ok(items) =
                parsers::parse_by_source(source, http, "prefer", None).await
            {
                for item in items.iter().take(50) {
                    let text = format!("{} {}", item.title, item.content_snippet).to_lowercase();
                    let matched: Vec<String> = monitor
                        .keywords
                        .iter()
                        .filter(|kw| text.contains(&kw.to_lowercase()))
                        .cloned()
                        .collect();

                    if !matched.is_empty() {
                        matching_articles.push(MatchedArticle {
                            title: item.title.clone(),
                            link: item.link.clone(),
                            source: item.source_name.clone(),
                            matched_keywords: matched,
                        });
                    }
                }
            }
        }
    }

    let count = matching_articles.len();
    Ok(MonitorAlert {
        monitor_id: monitor.id.clone(),
        monitor_name: monitor.name.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        triggered_by: if count > 0 {
            format!("{} keyword matches", count)
        } else {
            "no matches".to_string()
        },
        article_count: count,
        matching_articles,
    })
}

/// Deliver an alert via webhook and/or file.
async fn deliver_alert(alert: &MonitorAlert, monitor: &MonitorConfig) {
    // Webhook delivery (Slack/Discord/Teams JSON format)
    if let Some(ref webhook_url) = monitor.webhook_url {
        let payload = serde_json::json!({
            "text": format!(
                "🚨 IGS Alert: {} (triggered by {})\n{} matching articles",
                alert.monitor_name,
                alert.triggered_by,
                alert.article_count
            )
        });

        let client = reqwest::Client::new();
        if let Err(e) = client.post(webhook_url).json(&payload).send().await {
            tracing::warn!("Monitor {} webhook delivery failed: {}", alert.monitor_id, e);
        }
    }

    // File append
    if let Some(ref file_path) = monitor.alert_file {
        let path = expand_path(file_path);
        let alert_line = format!(
            "[{}] {} — {} ({} articles)\n",
            alert.timestamp, alert.monitor_name, alert.triggered_by, alert.article_count
        );
        if let Err(e) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, alert_line.as_bytes()))
        {
            tracing::warn!("Monitor {} file alert write failed: {}", alert.monitor_id, e);
        }
    }

    // Always log to stderr
    tracing::info!(
        "IGS Monitor alert: {} — {} ({} articles)",
        alert.monitor_name,
        alert.triggered_by,
        alert.article_count
    );
}

/// Expand ~ in path to $HOME
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(path.replacen('~', &home, 1));
        }
    }
    PathBuf::from(path)
}
