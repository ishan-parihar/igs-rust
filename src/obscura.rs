use crate::config;
use crate::types::ObscuraSettings;
use anyhow::{Context, Result};
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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

        if !self.binary_dir.exists() {
            std::fs::create_dir_all(&self.binary_dir)
                .context("Failed to create Obscura bin directory")?;
        }

        if self.binary_path.exists() && !self.should_check_update() {
            return Ok(self.binary_path.clone());
        }

        let latest_version = self.fetch_latest_version().await?;

        if self.binary_path.exists() {
            if let Ok(current) = self.read_version_file() {
                if current == latest_version {
                    self.write_last_check()?;
                    return Ok(self.binary_path.clone());
                }
            }
        }

        let arch = Self::detect_arch()?;
        let asset_name = format!("obscura-{}.tar.gz", arch);
        let url = format!("{}/{}/{}", GITHUB_DOWNLOAD_BASE, latest_version, asset_name);

        info!("Downloading Obscura {} from {}", latest_version, url);
        self.download_and_extract_binary(&url).await?;

        std::fs::write(&self.version_file, &latest_version)
            .context("Failed to write Obscura version file")?;
        self.write_last_check()?;

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
            Err(_) => true,
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

    /// Download and extract the binary from the given tar.gz URL
    /// Download and extract the binary from the given tar.gz URL
    async fn download_and_extract_binary(&self, url: &str) -> Result<()> {
        // Size limits
        const MAX_DOWNLOAD_SIZE: u64 = 200 * 1024 * 1024; // 200 MB
        const MAX_EXTRACTED_SIZE: u64 = 500 * 1024 * 1024; // 500 MB

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

        // Check Content-Length if available
        if let Some(len) = resp.content_length() {
            if len > MAX_DOWNLOAD_SIZE {
                anyhow::bail!(
                    "Download too large: {} bytes (max {})",
                    len,
                    MAX_DOWNLOAD_SIZE
                );
            }
        }

        // Download with size limit
        let bytes = resp
            .bytes()
            .await
            .context("Failed to download Obscura binary")?;
        if (bytes.len() as u64) > MAX_DOWNLOAD_SIZE {
            anyhow::bail!(
                "Download exceeded size limit: {} bytes (max {})",
                bytes.len(),
                MAX_DOWNLOAD_SIZE
            );
        }

        let temp_dir = tempfile::tempdir().context("Failed to create temp dir")?;
        let temp_path = temp_dir.path().join("obscura.tar.gz");
        tokio::fs::write(&temp_path, &bytes)
            .await
            .context("Failed to write Obscura archive")?;

        // Extract with size limit and path validation
        let tar_gz =
            std::fs::File::open(&temp_path).context("Failed to open downloaded archive")?;
        let tar_gz = flate2::read::GzDecoder::new(tar_gz);
        let tar_gz = tar_gz.take(MAX_EXTRACTED_SIZE); // Limit decompression
        let mut archive = tar::Archive::new(tar_gz);

        // Validate each entry before extracting
        for entry in archive
            .entries()
            .context("Failed to read archive entries")?
        {
            let entry = entry.context("Failed to read archive entry")?;
            let path = entry.path().context("Failed to get entry path")?;

            // Prevent path traversal
            let path = path.strip_prefix("").unwrap_or(&path);
            if path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                anyhow::bail!("Archive contains path traversal: {}", path.display());
            }
            if path.is_absolute() {
                anyhow::bail!("Archive contains absolute path: {}", path.display());
            }
        }

        // Rewind and extract
        let tar_gz = std::fs::File::open(&temp_path).context("Failed to reopen archive")?;
        let tar_gz = flate2::read::GzDecoder::new(tar_gz);
        let tar_gz = tar_gz.take(MAX_EXTRACTED_SIZE);
        let mut archive = tar::Archive::new(tar_gz);

        archive
            .unpack(&self.binary_dir)
            .context("Failed to extract Obscura archive")?;

        // Find and move the binary
        let extracted_binary = self.binary_dir.join("obscura");
        if extracted_binary.exists() {
            std::fs::rename(&extracted_binary, &self.binary_path)
                .context("Failed to move Obscura binary to final location")?;
        } else {
            for entry in std::fs::read_dir(&self.binary_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some("obscura") {
                    std::fs::rename(&path, &self.binary_path)
                        .context("Failed to move Obscura binary")?;
                    break;
                }
            }
        }

        // Make executable (unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.binary_path, std::fs::Permissions::from_mode(0o755))
                .context("Failed to set permissions")?;
        }

        // Cleanup (temp_dir is dropped automatically)
        Ok(())
    }

    /// Fetch with all available options including wait_selector.
    pub async fn fetch_with_all_options(
        &self,
        url: &str,
        dump_format: &str,
        _obey_robots: bool,
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

    /// Take a screenshot of a URL via CDP WebSocket.
    ///
    /// Starts Obscura in serve mode, connects via CDP, captures the page,
    /// returns base64-encoded image data, then shuts down the child process.
    /// Child process cleanup is guaranteed on all paths.
    pub async fn screenshot(
        &self,
        url: &str,
        format: &str,
        quality: Option<u32>,
        wait_until: &str,
    ) -> Result<String> {
        use futures_util::{SinkExt, StreamExt};
        use tokio::io::AsyncBufReadExt;

        let binary = self.ensure_ready().await?;
        let port = 9222
            + (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                % 1000) as u16;

        // Start Obscura in serve mode
        let mut cmd = tokio::process::Command::new(&binary);
        cmd.arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .arg("--headless")
            .arg(url);

        if let Some(ref proxy) = self.settings.proxy {
            cmd.arg("--proxy").arg(proxy);
        }

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("Failed to start Obscura serve")?;

        // Wait for the WebSocket debugger URL from stdout
        let ws_url = {
            let stdout = child.stdout.take().expect("stdout piped above");
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            let start = std::time::Instant::now();
            let mut found_url = None;
            while start.elapsed() < Duration::from_secs(30) {
                match tokio::time::timeout(Duration::from_secs(5), reader.next_line()).await {
                    Ok(Ok(Some(line))) => {
                        if line.contains("ws://") {
                            found_url = Some(line.trim().to_string());
                            break;
                        }
                    }
                    _ => break,
                }
            }
            match found_url {
                Some(url) => url,
                None => {
                    let _ = child.kill().await;
                    anyhow::bail!("Obscura did not emit a WebSocket URL within 30s");
                }
            }
        };

        // Connect to CDP
        let ws_result = tokio_tungstenite::connect_async(&ws_url).await;
        let (mut ws_stream, _) = match ws_result {
            Ok(v) => v,
            Err(e) => {
                let _ = child.kill().await;
                anyhow::bail!("Failed to connect to Obscura CDP WebSocket: {}", e);
            }
        };

        // Wait for page load based on wait_until parameter
        let wait_ms = match wait_until {
            "load" => 2000,
            "domcontentloaded" => 1500,
            "networkidle" => 4000,
            "done" => 5000,
            _ => 3000,
        };
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;

        // Build Page.captureScreenshot params — only include quality for jpeg
        let params = if format.eq_ignore_ascii_case("jpeg") {
            match quality {
                Some(q) => serde_json::json!({ "format": format, "quality": q }),
                None => serde_json::json!({ "format": format }),
            }
        } else {
            serde_json::json!({ "format": format })
        };

        let screenshot_msg = serde_json::json!({
            "id": 1,
            "method": "Page.captureScreenshot",
            "params": params,
        });

        let send_result = ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                screenshot_msg.to_string().into(),
            ))
            .await;
        if let Err(e) = send_result {
            let _ = child.kill().await;
            anyhow::bail!("Failed to send CDP command: {}", e);
        }

        // Read response — wait for result with matching id
        let mut screenshot_data = None;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            match tokio::time::timeout(Duration::from_secs(5), ws_stream.next()).await {
                Ok(Some(Ok(msg))) => {
                    if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if json.get("id").and_then(|i| i.as_u64()) == Some(1) {
                                if let Some(data) = json
                                    .get("result")
                                    .and_then(|r| r.get("data"))
                                    .and_then(|d| d.as_str())
                                {
                                    screenshot_data = Some(data.to_string());
                                    break;
                                }
                                if let Some(err) = json.get("error") {
                                    let err_msg = err
                                        .get("message")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("unknown CDP error");
                                    let _ = child.kill().await;
                                    anyhow::bail!("CDP error: {}", err_msg);
                                }
                            }
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    let _ = child.kill().await;
                    anyhow::bail!("WebSocket error: {}", e);
                }
                _ => continue,
            }
        }

        let _ = child.kill().await;

        screenshot_data.ok_or_else(|| anyhow::anyhow!("No screenshot data received from CDP"))
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
