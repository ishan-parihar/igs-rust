use crate::config;
use crate::types::ObscuraSettings;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::info;

/// Manages the Obscura headless browser binary lifecycle:
/// - Checks for updates once per day
/// - Downloads latest stable binary if not present or outdated
/// - Caches version metadata to avoid redundant API calls
/// - Provides path to the binary for subprocess invocation
pub struct ObscuraManager {
    binary_dir: PathBuf,
    binary_path: PathBuf,
    version_file: PathBuf,
    last_check_file: PathBuf,
    settings: ObscuraSettings,
}

const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/h4ckf0r0day/obscura/releases";
const GITHUB_DOWNLOAD_BASE: &str = "https://github.com/h4ckf0r0day/obscura/releases/download";
const CHECK_INTERVAL_SECS: u64 = 86400;

impl ObscuraManager {
    /// Create a new manager using the user config directory
    pub fn new(settings: &ObscuraSettings) -> Self {
        let bin_dir = config::user_config_dir().join("bin");
        Self {
            binary_path: bin_dir.join("obscura"),
            version_file: bin_dir.join(".obscura_version"),
            last_check_file: bin_dir.join(".obscura_last_check"),
            binary_dir: bin_dir,
            settings: settings.clone(),
        }
    }

    /// Ensure the Obscura binary is available and up-to-date.
    /// Checks at most once per day. Returns the path to the binary.
    pub async fn ensure_ready(&self) -> Result<PathBuf> {
        if !self.settings.enabled {
            anyhow::bail!("Obscura is not enabled. Set obscura.enabled=true in settings.yml");
        }

        // Create bin dir if needed
        if !self.binary_dir.exists() {
            std::fs::create_dir_all(&self.binary_dir)
                .context("Failed to create Obscura bin directory")?;
        }

        // Check if binary exists and if we need to check for updates
        if self.binary_path.exists() && !self.should_check_update() {
            return Ok(self.binary_path.clone());
        }

        // Fetch latest version from GitHub
        let latest_version = self.fetch_latest_version().await?;

        // Check if we already have this version
        if self.binary_path.exists() {
            if let Ok(current) = self.read_version_file() {
                if current == latest_version {
                    self.write_last_check()?;
                    return Ok(self.binary_path.clone());
                }
            }
        }

        // Download
        let arch = Self::detect_arch()?;
        let url = format!(
            "{}/{}/obscura-{}",
            GITHUB_DOWNLOAD_BASE, latest_version, arch
        );

        info!("Downloading Obscura {} from {}", latest_version, url);
        self.download_binary(&url).await?;

        // Write version metadata
        std::fs::write(&self.version_file, &latest_version)
            .context("Failed to write Obscura version file")?;
        self.write_last_check()?;

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.binary_path, std::fs::Permissions::from_mode(0o755))
                .context("Failed to make Obscura binary executable")?;
        }

        info!(
            "Obscura {} installed to {:?}",
            latest_version, self.binary_path
        );
        Ok(self.binary_path.clone())
    }

    /// Get the current installed version, if any
    pub fn installed_version(&self) -> Option<String> {
        if self.binary_path.exists() {
            self.read_version_file().ok()
        } else {
            None
        }
    }

    /// Check if we should check for updates (once per day)
    fn should_check_update(&self) -> bool {
        if !self.settings.auto_update {
            return false;
        }

        match std::fs::read_to_string(&self.last_check_file) {
            Ok(content) => {
                let last_check: u64 = content.trim().parse().unwrap_or(0);
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs();
                now.saturating_sub(last_check) >= CHECK_INTERVAL_SECS
            }
            Err(_) => true, // No check file = should check
        }
    }

    fn write_last_check(&self) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        std::fs::write(&self.last_check_file, now.to_string())
            .context("Failed to write last check file")?;
        Ok(())
    }

    fn read_version_file(&self) -> Result<String> {
        std::fs::read_to_string(&self.version_file)
            .context("Failed to read Obscura version file")
            .map(|s| s.trim().to_string())
    }

    /// Fetch the latest stable release version from GitHub API.
    /// Uses /releases (plural) and filters out prerelease tags.
    async fn fetch_latest_version(&self) -> Result<String> {
        let client = reqwest::Client::builder()
            .user_agent("igs-mcp/0.1")
            .build()?;

        let resp = client
            .get(GITHUB_RELEASES_API)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to fetch Obscura release info")?;

        if !resp.status().is_success() {
            anyhow::bail!("GitHub API returned status {}", resp.status());
        }

        let json: serde_json::Value = resp.json().await?;
        let releases = json
            .as_array()
            .context("Expected JSON array from releases API")?;

        // Find the first stable release (not prerelease, not draft)
        for release in releases {
            let tag = release["tag_name"].as_str().unwrap_or("");
            let is_prerelease = release["prerelease"].as_bool().unwrap_or(false);
            let is_draft = release["draft"].as_bool().unwrap_or(false);

            if is_draft || is_prerelease || tag.is_empty() {
                continue;
            }

            return Ok(tag.to_string());
        }

        anyhow::bail!("No stable release found for Obscura")
    }

    /// Download the binary from the given URL
    async fn download_binary(&self, url: &str) -> Result<()> {
        let client = reqwest::Client::builder()
            .user_agent("igs-mcp/0.1")
            .build()?;

        let resp = client
            .get(url)
            .send()
            .await
            .context("Failed to download Obscura binary")?;

        if !resp.status().is_success() {
            anyhow::bail!("Download returned status {}", resp.status());
        }

        let bytes = resp.bytes().await?;
        std::fs::write(&self.binary_path, &bytes).context("Failed to write Obscura binary")?;

        Ok(())
    }

    /// Fetch a URL using Obscura's fetch command.
    /// `dump_format` can be "markdown", "html", "text", or "semantic_tree".
    pub async fn fetch(&self, url: &str, dump_format: &str, obey_robots: bool) -> Result<String> {
        self.fetch_with_options(url, dump_format, obey_robots, "networkidle", false)
            .await
    }

    /// Fetch a URL with control over Obscura options.
    pub async fn fetch_with_options(
        &self,
        url: &str,
        dump_format: &str,
        obey_robots: bool,
        wait_until: &str,
        include_frames: bool,
    ) -> Result<String> {
        self.fetch_with_all_options(
            url,
            dump_format,
            obey_robots,
            wait_until,
            include_frames,
            None,
        )
        .await
    }

    /// Fetch with all available options including wait_selector.
    pub async fn fetch_with_all_options(
        &self,
        url: &str,
        dump_format: &str,
        obey_robots: bool,
        wait_until: &str,
        _include_frames: bool,
        wait_selector: Option<&str>,
    ) -> Result<String> {
        let binary = self.ensure_ready().await?;

        let mut cmd = tokio::process::Command::new(&binary);
        cmd.arg("fetch")
            .arg(url)
            .arg("--dump")
            .arg(dump_format)
            .arg("--wait-until")
            .arg(wait_until)
            .arg("--timeout")
            .arg((self.settings.timeout_ms / 1000).to_string());

        if self.settings.stealth {
            cmd.arg("--stealth");
        }

        if obey_robots {
            cmd.arg("--obey-robots");
        }

        if let Some(ref proxy) = self.settings.proxy {
            cmd.arg("--proxy").arg(proxy);
        }

        if let Some(selector) = wait_selector {
            cmd.arg("--selector").arg(selector);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute Obscura fetch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Obscura fetch failed: {}", stderr);
        }

        String::from_utf8(output.stdout).context("Obscura output was not valid UTF-8")
    }

    /// Scrape multiple URLs in parallel using Obscura's scrape command.
    pub async fn scrape_parallel(&self, urls: &[String], concurrency: u32) -> Result<String> {
        let binary = self.ensure_ready().await?;

        let mut cmd = tokio::process::Command::new(&binary);
        cmd.arg("scrape")
            .args(urls)
            .arg("--concurrency")
            .arg(concurrency.to_string())
            .arg("--format")
            .arg("json");

        if self.settings.stealth {
            cmd.arg("--stealth");
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute Obscura scrape")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Obscura scrape failed: {}", stderr);
        }

        String::from_utf8(output.stdout).context("Obscura output was not valid UTF-8")
    }

    /// Detect the current platform architecture for binary download
    fn detect_arch() -> Result<&'static str> {
        match (std::env::consts::ARCH, std::env::consts::OS) {
            ("x86_64", "linux") => Ok("x86_64-linux"),
            ("aarch64", "linux") => Ok("aarch64-linux"),
            ("x86_64", "macos") => Ok("x86_64-macos"),
            ("aarch64", "macos") => Ok("aarch64-macos"),
            _ => anyhow::bail!(
                "Unsupported platform for Obscura: {} {}",
                std::env::consts::ARCH,
                std::env::consts::OS
            ),
        }
    }
}
