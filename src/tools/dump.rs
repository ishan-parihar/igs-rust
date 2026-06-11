use std::fs;
use std::path::PathBuf;
use chrono::Utc;
use chrono::Datelike;
use crate::types::Settings;

/// Sanitize a subject string for use in file paths
fn sanitize_subject(subject: &str) -> String {
    let sanitized: String = subject.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            ' ' => '_',
            other => other,
        })
        .collect();
    // Limit to 100 chars, trim trailing dots/spaces
    let mut s = sanitized.chars().take(100).collect::<String>();
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.is_empty() { s = "unnamed".to_string(); }
    s
}

/// Expand ~ in path to $HOME
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(path.replacen('~', &home, 1))
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    }
}

/// Dump tool output as a markdown file if dump is enabled in settings
/// Path: {dump_dir}/{tool_name}/{YYYY}/{MM}/{DD}/{subject}.md
pub fn maybe_dump(settings: &Settings, tool_name: &str, subject: &str, body: &str) {
    if !settings.output.dump_enabled {
        return;
    }

    let now = Utc::now();
    let subject = sanitize_subject(subject);
    let base_dir = expand_path(&settings.output.dump_dir);

    let file_path = base_dir
        .join(tool_name)
        .join(format!("{:04}", now.year()))
        .join(format!("{:02}", now.month()))
        .join(format!("{:02}", now.day()))
        .join(format!("{}.md", subject));

    let frontmatter = format!(
        "---\ntool: {tool_name}\nsubject: {subject}\ntimestamp: {timestamp}\n---\n\n",
        tool_name = tool_name,
        subject = subject,
        timestamp = now.to_rfc3339()
    );

    let content = format!("{}{}", frontmatter, body);

    if let Some(parent) = file_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            tracing::warn!("Failed to create dump directory {}: {}", parent.display(), e);
            return;
        }
    }

    if let Err(e) = fs::write(&file_path, &content) {
        tracing::warn!("Failed to write dump file {}: {}", file_path.display(), e);
    }
}
