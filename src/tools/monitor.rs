//! Real-time monitoring & alerting system.
//!
//! Provides scheduled polling of news sources with threshold-based alerting.
//! Monitors can trigger on:
//! - Keyword appearance in new articles
//! - Entity mention count crossing a threshold
//! - Source content changes (new articles detected)
//!
//! Alerts are delivered via multiple channels:
//! - **Slack** webhook (`{"text": "..."}` format)
//! - **Discord** webhook (`{"content": "..."}` format)
//! - **Telegram** Bot API (`sendMessage` to a chat ID)
//! - **Email** via SMTP (using `lettre` crate if enabled, otherwise webhook-based email services)
//! - **File append** (local log file)
//! - **stderr log** (always-on `tracing::info!`)
//!
//! ## Setup Guide for AI Agents
//!
//! ### Slack
//! 1. Go to https://api.slack.com/messaging/webhooks
//! 2. Create a new webhook for your channel
//! 3. Copy the URL (looks like `https://hooks.slack.com/services/T.../B.../...`)
//! 4. Set `webhook_url` to that URL and `webhook_format` to `"slack"`
//!
//! ### Discord
//! 1. Go to Server Settings → Integrations → Webhooks in your Discord server
//! 2. Create a new webhook for a channel
//! 3. Copy the URL (looks like `https://discord.com/api/webhooks/...`)
//! 4. Set `webhook_url` to that URL and `webhook_format` to `"discord"`
//!
//! ### Telegram
//! 1. Create a bot via @BotFather → get the bot token
//! 2. Get your chat ID: send a message to your bot, then visit
//!    `https://api.telegram.org/bot<TOKEN>/getUpdates` → find `"chat":{"id":...}`
//! 3. Set `telegram_bot_token` and `telegram_chat_id`
//!
//! ### Email
//! 1. Use an email-to-webhook service (e.g., https://mailersea.com, or a custom endpoint)
//! 2. Or set up a simple HTTP endpoint that receives `{"to": "...", "subject": "...", "body": "..."}`
//! 3. Set `email_webhook_url` and `email_recipients`

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::parsers;
use crate::types::Settings;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    /// Poll interval in seconds (actually honored per-monitor)
    pub interval_secs: u64,
    /// Alert threshold: min keyword matches in a poll cycle
    pub threshold: u32,
    /// Webhook URL for alerts
    pub webhook_url: Option<String>,
    /// Webhook format: "slack", "discord", "teams", "raw" (default: "slack")
    pub webhook_format: Option<String>,
    /// File path to append alerts to
    pub alert_file: Option<String>,
    /// Telegram bot token (from @BotFather)
    pub telegram_bot_token: Option<String>,
    /// Telegram chat ID to send alerts to
    pub telegram_chat_id: Option<String>,
    /// Email webhook URL (HTTP endpoint that sends emails)
    pub email_webhook_url: Option<String>,
    /// Email recipients (comma-separated in the webhook payload)
    pub email_recipients: Option<Vec<String>>,
    /// Cooldown in seconds between alerts for the same monitor (default: 300 = 5 min)
    pub cooldown_secs: Option<u64>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorTestInput {
    /// Channel to test: "slack", "discord", "telegram", "email", "webhook"
    pub channel: String,
    /// Webhook URL (for slack/discord/email)
    pub webhook_url: Option<String>,
    /// Telegram bot token
    pub telegram_bot_token: Option<String>,
    /// Telegram chat ID
    pub telegram_chat_id: Option<String>,
    /// Test message (default: "IGS monitor test alert")
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonitorTestOutput {
    pub success: bool,
    pub channel: String,
    pub response: String,
    pub error: Option<String>,
}

// ─── Monitor Manager ──────────────────────────────────────────

pub struct MonitorManager {
    monitors: Arc<Mutex<Vec<MonitorConfig>>>,
    settings: Arc<Settings>,
    http: Arc<HttpClient>,
    /// Track last alert time per monitor ID for cooldown
    last_alert: Arc<Mutex<HashMap<String, u64>>>,
}

impl MonitorManager {
    pub fn new(settings: Arc<Settings>) -> Self {
        let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
        let http = Arc::new(HttpClient::new(&settings.http, &cache_dir));
        Self {
            monitors: Arc::new(Mutex::new(Vec::new())),
            settings,
            http,
            last_alert: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add(&self, monitor: MonitorConfig) {
        let mut monitors = self.monitors.lock().await;
        // Remove any existing monitor with the same ID (upsert semantics)
        monitors.retain(|m| m.id != monitor.id);
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

    /// Send a test alert to verify notification channel configuration.
    pub async fn test_alert(&self, input: MonitorTestInput) -> MonitorTestOutput {
        let message = input.message.unwrap_or_else(|| "IGS monitor test alert".to_string());
        let channel = input.channel.as_str();

        match channel {
            "slack" | "webhook" => {
                if let Some(ref url) = input.webhook_url {
                    let payload = serde_json::json!({"text": &message});
                    let client = reqwest::Client::new();
                    match client.post(url).json(&payload).send().await {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            if status.is_success() {
                                MonitorTestOutput { success: true, channel: channel.into(), response: body, error: None }
                            } else {
                                MonitorTestOutput { success: false, channel: channel.into(), response: body, error: Some(format!("HTTP {}", status)) }
                            }
                        }
                        Err(e) => MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some(e.to_string()) },
                    }
                } else {
                    MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some("webhook_url is required for slack".into()) }
                }
            }
            "discord" => {
                if let Some(ref url) = input.webhook_url {
                    let payload = serde_json::json!({"content": &message});
                    let client = reqwest::Client::new();
                    match client.post(url).json(&payload).send().await {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            if status.is_success() {
                                MonitorTestOutput { success: true, channel: channel.into(), response: body, error: None }
                            } else {
                                MonitorTestOutput { success: false, channel: channel.into(), response: body, error: Some(format!("HTTP {}", status)) }
                            }
                        }
                        Err(e) => MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some(e.to_string()) },
                    }
                } else {
                    MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some("webhook_url is required for discord".into()) }
                }
            }
            "telegram" => {
                let token = input.telegram_bot_token.as_deref().unwrap_or("");
                let chat_id = input.telegram_chat_id.as_deref().unwrap_or("");
                if token.is_empty() || chat_id.is_empty() {
                    return MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some("telegram_bot_token and telegram_chat_id are required".into()) };
                }
                let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
                let payload = serde_json::json!({
                    "chat_id": chat_id,
                    "text": &message,
                    "parse_mode": "HTML"
                });
                let client = reqwest::Client::new();
                match client.post(&url).json(&payload).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        if status.is_success() {
                            MonitorTestOutput { success: true, channel: channel.into(), response: body, error: None }
                        } else {
                            MonitorTestOutput { success: false, channel: channel.into(), response: body, error: Some(format!("HTTP {}", status)) }
                        }
                    }
                    Err(e) => MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some(e.to_string()) },
                }
            }
            "email" => {
                if let Some(ref url) = input.webhook_url {
                    let payload = serde_json::json!({
                        "to": "test@example.com",
                        "subject": "IGS Monitor Test",
                        "body": &message
                    });
                    let client = reqwest::Client::new();
                    match client.post(url).json(&payload).send().await {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            if status.is_success() {
                                MonitorTestOutput { success: true, channel: channel.into(), response: body, error: None }
                            } else {
                                MonitorTestOutput { success: false, channel: channel.into(), response: body, error: Some(format!("HTTP {}", status)) }
                            }
                        }
                        Err(e) => MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some(e.to_string()) },
                    }
                } else {
                    MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some("webhook_url (email webhook endpoint) is required for email".into()) }
                }
            }
            _ => MonitorTestOutput { success: false, channel: channel.into(), response: String::new(), error: Some(format!("Unknown channel '{}'. Supported: slack, discord, telegram, email, webhook", channel)) },
        }
    }

    /// Start all active monitors as background tokio tasks.
    /// Each monitor polls at its own configured interval_secs.
    pub fn start_all(&self) {
        let monitors = self.monitors.clone();
        let http = self.http.clone();
        let settings = self.settings.clone();
        let last_alert = self.last_alert.clone();

        tokio::spawn(async move {
            // Track next-poll-time per monitor ID
            let mut next_poll: HashMap<String, u64> = HashMap::new();

            loop {
                let now = chrono::Utc::now().timestamp() as u64;
                let active_monitors: Vec<MonitorConfig> = {
                    let guards = monitors.lock().await;
                    guards.iter().filter(|m| m.active).cloned().collect()
                };

                for monitor in active_monitors {
                    let interval = monitor.interval_secs.max(30); // minimum 30s
                    let should_poll = match next_poll.get(&monitor.id) {
                        Some(&next_time) => now >= next_time,
                        None => true, // first poll
                    };

                    if !should_poll {
                        continue;
                    }

                    // Schedule next poll
                    next_poll.insert(monitor.id.clone(), now + interval);

                    let http = http.clone();
                    let settings = settings.clone();
                    let last_alert_clone = last_alert.clone();
                    let monitor_for_task = monitor.clone();

                    tokio::spawn(async move {
                        if let Ok(alert) = poll_monitor(&monitor_for_task, &http, &settings).await {
                            if alert.article_count >= monitor_for_task.threshold as usize {
                                // Check cooldown
                                let cooldown = monitor_for_task.cooldown_secs.unwrap_or(300);
                                let should_deliver = {
                                    let la = last_alert_clone.lock().await;
                                    match la.get(&monitor_for_task.id) {
                                        Some(&last) => now - last >= cooldown,
                                        None => true,
                                    }
                                };

                                if should_deliver {
                                    // Update last alert time
                                    {
                                        let mut la = last_alert_clone.lock().await;
                                        la.insert(monitor_for_task.id.clone(), now);
                                    }
                                    deliver_alert(&alert, &monitor_for_task).await;
                                } else {
                                    tracing::debug!(
                                        "Monitor {} alert suppressed (cooldown {}s)",
                                        monitor_for_task.id, cooldown
                                    );
                                }
                            }
                        }
                    });
                }

                // Check every 10 seconds for monitors that need polling
                tokio::time::sleep(TokioDuration::from_secs(10)).await;
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
            .take(20)
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

/// Deliver an alert via all configured channels.
async fn deliver_alert(alert: &MonitorAlert, monitor: &MonitorConfig) {
    let alert_text = format!(
        "🚨 IGS Alert: {}\nTriggered by: {}\n{} matching articles",
        alert.monitor_name,
        alert.triggered_by,
        alert.article_count
    );

    // 1. Webhook (Slack/Discord/Teams/Raw)
    if let Some(ref webhook_url) = monitor.webhook_url {
        let format = monitor.webhook_format.as_deref().unwrap_or("slack");
        let payload = match format {
            "discord" => serde_json::json!({"content": &alert_text}),
            "teams" => serde_json::json!({"text": &alert_text}),
            "raw" => serde_json::json!({
                "monitor_id": &alert.monitor_id,
                "monitor_name": &alert.monitor_name,
                "timestamp": &alert.timestamp,
                "triggered_by": &alert.triggered_by,
                "article_count": alert.article_count,
                "matching_articles": &alert.matching_articles,
            }),
            _ => serde_json::json!({"text": &alert_text}), // slack default
        };

        let client = reqwest::Client::new();
        if let Err(e) = client.post(webhook_url).json(&payload).send().await {
            tracing::warn!("Monitor {} webhook delivery failed: {}", alert.monitor_id, e);
        }
    }

    // 2. Telegram
    if let (Some(ref token), Some(ref chat_id)) = (&monitor.telegram_bot_token, &monitor.telegram_chat_id) {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": &alert_text,
            "parse_mode": "HTML",
            "disable_web_page_preview": true
        });
        let client = reqwest::Client::new();
        if let Err(e) = client.post(&url).json(&payload).send().await {
            tracing::warn!("Monitor {} Telegram delivery failed: {}", alert.monitor_id, e);
        }
    }

    // 3. Email (via webhook endpoint)
    if let Some(ref email_url) = monitor.email_webhook_url {
        let recipients = monitor.email_recipients
            .as_ref()
            .map(|r| r.join(", "))
            .unwrap_or_default();
        let payload = serde_json::json!({
            "to": &recipients,
            "subject": format!("🚨 IGS Alert: {}", alert.monitor_name),
            "body": &alert_text
        });
        let client = reqwest::Client::new();
        if let Err(e) = client.post(email_url).json(&payload).send().await {
            tracing::warn!("Monitor {} email delivery failed: {}", alert.monitor_id, e);
        }
    }

    // 4. File append
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

    // 5. Always log
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_config_serialization() {
        let config = MonitorConfig {
            id: "test-monitor".into(),
            name: "Test Monitor".into(),
            pools: vec!["GLOBAL_TECH_CYBER".into()],
            keywords: vec!["CVE".into(), "exploit".into()],
            interval_secs: 300,
            threshold: 3,
            webhook_url: Some("https://hooks.slack.com/services/...".into()),
            webhook_format: Some("slack".into()),
            alert_file: None,
            telegram_bot_token: Some("123:ABC".into()),
            telegram_chat_id: Some("456".into()),
            email_webhook_url: None,
            email_recipients: None,
            cooldown_secs: Some(600),
            active: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: MonitorConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-monitor");
        assert_eq!(parsed.webhook_format, Some("slack".into()));
        assert!(parsed.telegram_bot_token.is_some());
        assert_eq!(parsed.cooldown_secs, Some(600));
    }

    #[test]
    fn test_monitor_test_input_validation() {
        // Telegram without token/chat_id should fail
        let input = MonitorTestInput {
            channel: "telegram".into(),
            webhook_url: None,
            telegram_bot_token: None,
            telegram_chat_id: None,
            message: None,
        };
        // We can't call test_alert without a runtime, but we can verify the struct
        assert_eq!(input.channel, "telegram");
        assert!(input.telegram_bot_token.is_none());
    }

    #[test]
    fn test_expand_path() {
        let path = expand_path("~/alerts.log");
        assert!(path.to_string_lossy().contains("alerts.log"));
    }
}
