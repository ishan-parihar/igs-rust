use crate::error::{AppError, AppResult};
use crate::types::*;
use std::path::{Path, PathBuf};
use tokio::fs;

// Embedded default configs — compiled into the binary so bootstrap works
// even when the source tree isn't present (e.g., installed via curl).
const DEFAULT_SETTINGS: &str = include_str!("../config/settings.yml");
const DEFAULT_POOLS: &str = include_str!("../config/pools.yml");
const DEFAULT_SOURCES: &str = include_str!("../config/sources.yml");
const DEFAULT_COUNTRIES: &str = include_str!("../config/countries.yml");

const EMBEDDED_DEFAULTS: &[(&str, &str)] = &[
    ("settings.yml", DEFAULT_SETTINGS),
    ("pools.yml", DEFAULT_POOLS),
    ("sources.yml", DEFAULT_SOURCES),
    ("countries.yml", DEFAULT_COUNTRIES),
];

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
    let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home));
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

async fn ensure_bootstrapped() -> AppResult<()> {
    let user_dir = user_config_dir();
    let pkg_dir = package_config_dir();
    fs::create_dir_all(&user_dir)
        .await
        .map_err(AppError::from)?;

    for &(name, embedded) in EMBEDDED_DEFAULTS {
        let target = user_dir.join(name);
        if !file_exists(&target).await {
            // Try filesystem first (source tree), fall back to embedded default.
            let content = {
                let src = pkg_dir.join(name);
                if file_exists(&src).await {
                    fs::read(&src).await.map_err(AppError::from)?
                } else {
                    embedded.as_bytes().to_vec()
                }
            };
            fs::write(&target, &content).await.map_err(AppError::from)?;
            tracing::info!("Bootstrapped {}", name);
        }
    }

    // Migrate stale settings.yml files that predate new top-level sections.
    // This is additive-only — existing values are never touched.
    migrate_settings().await?;

    Ok(())
}

/// Inject missing top-level sections into an existing settings.yml.
///
/// Uses a text-level scan (not YAML parse+serialize) so user comments,
/// ordering, and formatting are completely preserved. Only the missing
/// stub is appended at the end of the file.
///
/// Currently handles:
/// - `reddit:` — added in v1.0.0; users bootstrapped before that lack it
async fn migrate_settings() -> AppResult<()> {
    let settings_file = user_config_dir().join("settings.yml");
    if !file_exists(&settings_file).await {
        return Ok(()); // will be written by the bootstrap loop above
    }

    let content = fs::read_to_string(&settings_file)
        .await
        .map_err(|e| AppError::from(format!("Migration read failed: {}", e)))?;

    // Each entry: (top-level YAML key, stub to append when absent).
    // The stub must start with a newline so it appends cleanly regardless
    // of whether the file ends with a trailing newline.
    let migrations: &[(&str, &str)] = &[
        (
            "reddit:",
            concat!(
                "\n",
                "# \u{2500}\u{2500}\u{2500} Reddit Integration \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n",
                "# reddit.search and reddit.feed use cookie-based auth to bypass Akamai bot detection.\n",
                "# Without a valid cookie both tools will return a clear error explaining what to do.\n",
                "#\n",
                "# How to get your Reddit cookie:\n",
                "#   1. Open reddit.com in your browser and sign in (or use as guest)\n",
                "#   2. Open DevTools -> Network tab -> refresh the page\n",
                "#   3. Click any request to reddit.com -> Headers -> copy the 'Cookie:' request header value\n",
                "#   4. Paste it below (or set IGS_REDDIT_COOKIE env var and use: cookie: \"${IGS_REDDIT_COOKIE}\")\n",
                "reddit:\n",
                "  enabled: false\n",
                "  cookie: \"\"\n",
            ),
        ),
    ];

    let mut appended: Vec<&str> = Vec::new();

    // Build the new content incrementally; only write if something changed.
    let mut new_content = content.clone();
    for &(key, stub) in migrations {
        // A top-level key appears at column 0, not indented.
        // Match `key` at the start of any line (handles both "reddit:" and "reddit: ...").
        let present = new_content
            .lines()
            .any(|line| line.starts_with(key) || line == key.trim_end_matches(':'));
        if !present {
            new_content.push_str(stub);
            appended.push(key);
        }
    }

    if !appended.is_empty() {
        tracing::info!(
            "settings.yml migration: added missing section(s): {}",
            appended.join(", ")
        );
        fs::write(&settings_file, new_content.as_bytes())
            .await
            .map_err(|e| AppError::from(format!("Migration write failed: {}", e)))?;
    }

    Ok(())
}

/// Replace ${VAR_NAME} patterns with environment variable values.
/// Leaves literal ${VAR} if env var is unset (graceful fallback).
/// Shared between `config::expand_env_vars` (async path) and
/// `server::load_settings_sync` (sync path).
pub fn expand_env_vars(s: &str) -> String {
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

async fn read_yaml<T: serde::de::DeserializeOwned>(file: &Path) -> AppResult<T> {
    let raw = fs::read_to_string(file)
        .await
        .map_err(|e| AppError::from(format!("Failed to read {}: {}", file.display(), e)))?;
    let expanded = expand_env_vars(&raw);
    let doc: T = serde_yaml::from_str(&expanded)
        .map_err(|e| AppError::config(format!("Failed to parse {}: {}", file.display(), e)))?;
    Ok(doc)
}

async fn write_yaml<T: serde::Serialize>(file: &Path, data: &T) -> AppResult<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).await.map_err(|e| {
            AppError::from(format!("Failed to create dir {}: {}", parent.display(), e))
        })?;
    }
    let txt = serde_yaml::to_string(data)
        .map_err(|e| AppError::config(format!("Failed to serialize: {}", e)))?;
    fs::write(file, txt.as_bytes())
        .await
        .map_err(|e| AppError::from(format!("Failed to write {}: {}", file.display(), e)))?;
    Ok(())
}

async fn merge_missing_default_sources() -> AppResult<()> {
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
    let mut added: Vec<String> = Vec::new();

    for src in &default_doc.sources {
        if !user_ids.contains(&src.id) {
            merged.push(src.clone());
            added.push(src.id.clone());
        }
    }

    if !added.is_empty() {
        tracing::info!(
            "Merging {} missing default source(s) into {}: {}",
            added.len(),
            user_file.display(),
            added.join(", ")
        );
        let merged_file = SourcesFile { sources: merged };
        write_yaml(&user_file, &merged_file).await?;
    }
    Ok(())
}

/// Load pool definitions from `pools.yml`.
pub async fn load_pools() -> AppResult<PoolsFile> {
    ensure_bootstrapped().await?;
    let file = user_config_dir().join("pools.yml");
    let parsed: PoolsFile = read_yaml(&file).await?;
    Ok(parsed)
}

/// Save pool definitions to `pools.yml`.
pub async fn save_pools(data: &PoolsFile) -> AppResult<()> {
    let file = user_config_dir().join("pools.yml");
    write_yaml(&file, data).await?;
    Ok(())
}

/// Load source definitions from `sources.yml`, merging any missing defaults.
pub async fn load_sources() -> AppResult<SourcesFile> {
    ensure_bootstrapped().await?;
    merge_missing_default_sources().await?;
    let file = user_config_dir().join("sources.yml");
    let parsed: SourcesFile = read_yaml(&file).await?;
    Ok(parsed)
}

/// Save source definitions to `sources.yml`.
pub async fn save_sources(data: &SourcesFile) -> AppResult<()> {
    let file = user_config_dir().join("sources.yml");
    write_yaml(&file, data).await?;
    Ok(())
}

/// Load application settings from `settings.yml`.
pub async fn load_settings() -> AppResult<Settings> {
    ensure_bootstrapped().await?;
    let file = user_config_dir().join("settings.yml");
    let parsed: Settings = read_yaml(&file).await?;
    Ok(parsed)
}

/// Load country definitions from `countries.yml`.
pub async fn load_countries() -> AppResult<serde_json::Value> {
    ensure_bootstrapped().await?;
    let user_file = user_config_dir().join("countries.yml");
    let content = if file_exists(&user_file).await {
        fs::read_to_string(&user_file)
            .await
            .map_err(AppError::from)?
    } else {
        let pkg_file = package_config_dir().join("countries.yml");
        if file_exists(&pkg_file).await {
            fs::read_to_string(&pkg_file)
                .await
                .map_err(AppError::from)?
        } else {
            return Ok(serde_json::json!({"countries": []}));
        }
    };
    let val: serde_json::Value = serde_yaml::from_str(&content)
        .map_err(|e| AppError::config(format!("Failed to parse YAML: {}", e)))?;
    Ok(val)
}
