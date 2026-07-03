//! P3: Plugin & extension system + scheduled reports.
//!
//! Provides:
//! - Webhook enrichment: POST articles to an external NLP/ML service and get enriched JSON back
//! - Script hooks: pipe article text through an external script (Python, Node, etc.)
//! - Scheduled report generation: cron-like config for automated briefings
//! - Export: save articles/reports to file in various formats

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types_base::OutputOptions;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Webhook Enrichment ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebhookEnrichInput {
    /// Webhook URL to POST articles to
    pub webhook_url: String,
    /// Articles as JSON array to send to the webhook
    pub articles_json: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebhookEnrichOutput {
    pub success: bool,
    pub enriched_count: usize,
    pub response: String,
    pub error: Option<String>,
}

/// POST articles to an external webhook for enrichment (e.g., an LLM service,
/// a custom NLP pipeline, or a third-party API). The webhook should return
/// enriched JSON that replaces the original articles.
pub async fn webhook_enrich(input: WebhookEnrichInput) -> Result<WebhookEnrichOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let body = serde_json::json!({
        "articles": serde_json::from_str::<serde_json::Value>(&input.articles_json)
            .map_err(|e| format!("Invalid articles_json: {}", e))?,
    });

    let outcome = http
        .post_json(&input.webhook_url, &body, None)
        .await
        .map_err(|e| format!("Webhook request failed: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("post_json never returns Cached") };

    if resp.status >= 400 {
        return Ok(WebhookEnrichOutput {
            success: false,
            enriched_count: 0,
            response: resp.body_text,
            error: Some(format!("HTTP {}", resp.status)),
        });
    }

    // Try to parse the response as enriched articles
    let enriched_count = serde_json::from_str::<serde_json::Value>(&resp.body_text)
        .ok()
        .and_then(|v| {
            v.get("articles")
                .and_then(|a| a.as_array())
                .map(|a| a.len())
                .or_else(|| v.as_array().map(|a| a.len()))
        })
        .unwrap_or(0);

    Ok(WebhookEnrichOutput {
        success: true,
        enriched_count,
        response: resp.body_text,
        error: None,
    })
}

// ─── Script Hook ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScriptHookInput {
    /// Script command (e.g., "python3 enrich.py" or "node process.js")
    pub command: String,
    /// Text to pipe to the script's stdin
    pub text: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScriptHookOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Pipe text through an external script. The script receives the text on stdin
/// and should output the processed result on stdout. This enables integration
/// with Python/Node/Bash NLP pipelines without modifying IGS itself.
pub async fn script_hook(input: ScriptHookInput) -> Result<ScriptHookOutput, String> {
    let parts: Vec<&str> = input.command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".into());
    }

    let mut cmd = tokio::process::Command::new(parts[0]);
    for arg in &parts[1..] {
        cmd.arg(arg);
    }
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn script: {}", e))?;

    // Write text to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(input.text.as_bytes()).await
            .map_err(|e| format!("Failed to write to script stdin: {}", e))?;
        // stdin is dropped here, closing the pipe
    }

    let output = child.wait_with_output().await
        .map_err(|e| format!("Script execution failed: {}", e))?;

    Ok(ScriptHookOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code(),
    })
}

// ─── Export ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportInput {
    /// Data to export (JSON string)
    pub data_json: String,
    /// Output file path
    pub file_path: String,
    /// Format: "json", "toon", "markdown"
    pub format: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportOutput {
    pub success: bool,
    pub file_path: String,
    pub bytes_written: usize,
    pub error: Option<String>,
}

/// Export data to a file in the specified format.
pub async fn export_data(input: ExportInput) -> Result<ExportOutput, String> {
    let format = input.format.as_deref().unwrap_or("json");
    let content = match format {
        "toon" => {
            let value: serde_json::Value = serde_json::from_str(&input.data_json)
                .map_err(|e| format!("Invalid JSON for TOON export: {}", e))?;
            crate::tools::helpers::toon_encode(&value)
        }
        "markdown" | "md" => {
            let value: serde_json::Value = serde_json::from_str(&input.data_json)
                .map_err(|e| format!("Invalid JSON for markdown export: {}", e))?;
            format_json_as_markdown(&value)
        }
        _ => {
            // "json" — pretty-print
            let value: serde_json::Value = serde_json::from_str(&input.data_json)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            serde_json::to_string_pretty(&value).unwrap_or_default()
        }
    };

    let path = expand_path(&input.file_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let bytes = content.as_bytes();
    std::fs::write(&path, bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(ExportOutput {
        success: true,
        file_path: input.file_path,
        bytes_written: bytes.len(),
        error: None,
    })
}

// ─── Scheduled Reports ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScheduledReport {
    pub name: String,
    /// Cron-like schedule (e.g., "0 9 * * *" = daily at 9am)
    pub schedule: String,
    /// Pool IDs to include
    pub pools: Vec<String>,
    /// Output file path template ({{date}} is replaced)
    pub output_template: String,
    /// Report style: brief, detailed, bullet
    pub style: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScheduleReportInput {
    pub name: String,
    pub schedule: String,
    /// Comma-separated pool IDs
    pub pools: Vec<String>,
    pub output_template: String,
    pub style: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScheduleReportOutput {
    pub created: bool,
    pub name: String,
}

/// Format a JSON value as a simple markdown document.
fn format_json_as_markdown(value: &serde_json::Value) -> String {
    let mut md = String::new();
    match value {
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                md.push_str(&format!("## Item {}\n\n", i + 1));
                md.push_str(&format_json_as_markdown(item));
                md.push('\n');
            }
        }
        serde_json::Value::Object(obj) => {
            for (key, val) in obj {
                md.push_str(&format!("- **{}:** {}\n", key, val));
            }
        }
        _ => {
            md.push_str(&value.to_string());
        }
    }
    md
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
    fn test_expand_path() {
        let path = expand_path("~/test");
        assert!(path.to_string_lossy().contains("test"));
    }

    #[test]
    fn test_format_json_as_markdown_object() {
        let value = serde_json::json!({"name": "test", "count": 42});
        let md = format_json_as_markdown(&value);
        assert!(md.contains("**name:**"));
        assert!(md.contains("**count:**"));
    }

    #[test]
    fn test_format_json_as_markdown_array() {
        let value = serde_json::json!([{"a": 1}, {"b": 2}]);
        let md = format_json_as_markdown(&value);
        assert!(md.contains("## Item 1"));
        assert!(md.contains("## Item 2"));
    }

    #[test]
    fn test_scheduled_report_serialization() {
        let report = ScheduledReport {
            name: "daily-brief".into(),
            schedule: "0 9 * * *".into(),
            pools: vec!["GLOBAL_TECH_CYBER".into()],
            output_template: "~/briefs/{{date}}.md".into(),
            style: "brief".into(),
            active: true,
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: ScheduledReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "daily-brief");
        assert!(parsed.active);
    }
}
