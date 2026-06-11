use crate::types::*;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Determine the user config directory.
/// Precedence:
/// 1. env IGS_CONFIG_DIR
/// 2. $XDG_CONFIG_HOME/igs-mcp or ~/.config/igs-mcp
pub fn user_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("IGS_CONFIG_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", home));
    PathBuf::from(xdg).join("igs-mcp")
}

/// Resolve the package config directory (where default config files ship).
pub fn package_config_dir() -> PathBuf {
    // Resolve relative to the executable or CWD
    let cwd = std::env::current_dir().unwrap_or_default();
    cwd.join("config")
}

async fn file_exists(p: &Path) -> bool {
    fs::metadata(p).await.is_ok()
}

async fn ensure_bootstrapped() -> Result<()> {
    let user_dir = user_config_dir();
    let pkg_dir = package_config_dir();
    fs::create_dir_all(&user_dir).await?;

    for f in &["pools.yml", "sources.yml", "settings.yml", "countries.yml"] {
        let target = user_dir.join(f);
        if !file_exists(&target).await {
            let src = pkg_dir.join(f);
            if file_exists(&src).await {
                let content = fs::read(&src).await?;
                fs::write(&target, &content).await?;
                tracing::info!("Bootstrapped {} from package config", f);
            }
        }
    }
    Ok(())
}

/// Replace ${VAR_NAME} patterns with environment variable values.
/// Leaves literal ${VAR} if env var is unset (graceful fallback).
fn expand_env_vars(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let mut var_name = String::new();
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
                var_name.push(c);
            }
            match std::env::var(&var_name) {
                Ok(val) => result.push_str(&val),
                Err(_) => {
                    result.push('$');
                    result.push('{');
                    result.push_str(&var_name);
                    result.push('}');
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

async fn read_yaml<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T> {
    let raw = fs::read_to_string(file)
        .await
        .with_context(|| format!("Failed to read {}", file.display()))?;
    let expanded = expand_env_vars(&raw);
    let doc: T = serde_yaml::from_str(&expanded)
        .with_context(|| format!("Failed to parse {}", file.display()))?;
    Ok(doc)
}

async fn write_yaml<T: serde::Serialize>(file: &Path, data: &T) -> Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).await?;
    }
    let txt = serde_yaml::to_string(data)?;
    fs::write(file, txt.as_bytes()).await?;
    Ok(())
}

async fn merge_missing_default_sources() -> Result<()> {
    let user_file = user_config_dir().join("sources.yml");
    let default_file = package_config_dir().join("sources.yml");
    if !file_exists(&user_file).await || !file_exists(&default_file).await {
        return Ok(());
    }

    let user_doc: SourcesFile = read_yaml(&user_file).await?;
    let default_doc: SourcesFile = read_yaml(&default_file).await?;

    let user_ids: std::collections::HashSet<String> =
        user_doc.sources.iter().map(|s| s.id.clone()).collect();

    let mut merged = user_doc.sources.clone();
    let mut changed = false;

    for src in &default_doc.sources {
        if !user_ids.contains(&src.id) {
            merged.push(src.clone());
            changed = true;
        }
    }

    if changed {
        let merged_file = SourcesFile { sources: merged };
        write_yaml(&user_file, &merged_file).await?;
    }
    Ok(())
}

pub async fn load_pools() -> Result<PoolsFile> {
    ensure_bootstrapped().await?;
    let file = user_config_dir().join("pools.yml");
    let parsed: PoolsFile = read_yaml(&file).await?;
    Ok(parsed)
}

pub async fn save_pools(data: &PoolsFile) -> Result<()> {
    let file = user_config_dir().join("pools.yml");
    write_yaml(&file, data).await?;
    Ok(())
}

pub async fn load_sources() -> Result<SourcesFile> {
    ensure_bootstrapped().await?;
    merge_missing_default_sources().await?;
    let file = user_config_dir().join("sources.yml");
    let parsed: SourcesFile = read_yaml(&file).await?;
    Ok(parsed)
}

pub async fn save_sources(data: &SourcesFile) -> Result<()> {
    let file = user_config_dir().join("sources.yml");
    write_yaml(&file, data).await?;
    Ok(())
}

pub async fn load_settings() -> Result<Settings> {
    ensure_bootstrapped().await?;
    let file = user_config_dir().join("settings.yml");
    let parsed: Settings = read_yaml(&file).await?;
    Ok(parsed)
}

pub async fn load_countries() -> Result<serde_json::Value> {
    ensure_bootstrapped().await?;
    let user_file = user_config_dir().join("countries.yml");
    let content = if file_exists(&user_file).await {
        fs::read_to_string(&user_file).await?
    } else {
        let pkg_file = package_config_dir().join("countries.yml");
        if file_exists(&pkg_file).await {
            fs::read_to_string(&pkg_file).await?
        } else {
            return Ok(serde_json::json!({"countries": []}));
        }
    };
    let val: serde_json::Value = serde_yaml::from_str(&content)?;
    Ok(val)
}
