use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};

// ─── JSON-RPC 2.0 Types ────────────────────────────────────────

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// ─── MCP Tool Result Types ─────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

// ─── Lightpanda MCP Client ─────────────────────────────────────

/// Manages a persistent Lightpanda MCP server subprocess.
/// Spawns `lightpanda mcp` on first use, communicates via JSON-RPC 2.0.
/// Maintains page state between calls within a session.
pub struct LightpandaMcpClient {
    binary_path: PathBuf,
    inner: Arc<Mutex<LightpandaMcpInner>>,
}

struct LightpandaMcpInner {
    child: Option<Child>,
    stdin_tx: Option<tokio::sync::mpsc::Sender<String>>,
    pending: Vec<(u64, oneshot::Sender<JsonRpcResponse>)>,
    next_id: u64,
    initialized: bool,
}

impl LightpandaMcpClient {
    pub fn new(binary_path: PathBuf) -> Self {
        Self {
            binary_path,
            inner: Arc::new(Mutex::new(LightpandaMcpInner {
                child: None,
                stdin_tx: None,
                pending: Vec::new(),
                next_id: 1,
                initialized: false,
            })),
        }
    }

    /// Ensure the subprocess is running and MCP session is initialized.
    async fn ensure_running(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;

        // Check if already running
        if let Some(ref mut child) = inner.child {
            if child.try_wait()?.is_none()
                && inner.initialized {
                    return Ok(());
                }
            // Process exited — clean up
            inner.child = None;
            inner.stdin_tx = None;
            inner.initialized = false;
        }

        // Spawn fresh subprocess
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("mcp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn()
            .context("Failed to spawn lightpanda mcp — is the binary installed?")?;

        let stdin = child.stdin.take().context("No stdin")?;
        let stdout = child.stdout.take().context("No stdout")?;

        // Channel for writing to stdin
        let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::channel::<String>(64);

        // Writer task: reads from channel, writes to stdin
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = stdin_rx.recv().await {
                if stdin.write_all(msg.as_bytes()).await.is_err() { break; }
                if stdin.write_all(b"\n").await.is_err() { break; }
                if stdin.flush().await.is_err() { break; }
            }
        });

        // Reader task: reads from stdout, dispatches responses
        let inner_clone = self.inner.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }
                let resp: JsonRpcResponse = match serde_json::from_str(trimmed) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                if let Some(id) = resp.id {
                    let mut inner = inner_clone.lock().await;
                    if let Some(pos) = inner.pending.iter().position(|(pid, _)| *pid == id) {
                        let (_, sender) = inner.pending.remove(pos);
                        let _ = sender.send(resp);
                    }
                }
            }
        });

        inner.child = Some(child);
        inner.stdin_tx = Some(stdin_tx.clone());

        // Send initialize
        let id = inner.next_id;
        inner.next_id += 1;

        let init_req = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: "initialize".into(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "igs-mcp", "version": "0.2.0" }
            })),
        };

        let (tx_init, rx_init) = oneshot::channel();
        inner.pending.push((id, tx_init));
        let msg = serde_json::to_string(&init_req)?;
        stdin_tx.send(msg).await.map_err(|_| anyhow::anyhow!("channel closed"))?;

        // Drop the lock while waiting for response
        drop(inner);

        let _resp = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            rx_init,
        ).await.context("Timeout waiting for MCP initialize")??;

        // Send initialized notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let mut inner = self.inner.lock().await;
        let tx = inner.stdin_tx.as_ref().context("no stdin channel")?;
        tx.send(serde_json::to_string(&notification)?).await
            .map_err(|_| anyhow::anyhow!("channel closed"))?;

        inner.initialized = true;
        tracing::info!("Lightpanda MCP session initialized");
        Ok(())
    }

    /// Call a tool on the Lightpanda MCP server.
    pub async fn call_tool(&self, tool_name: &str, arguments: serde_json::Value) -> Result<McpToolResult> {
        self.ensure_running().await?;

        let mut inner = self.inner.lock().await;
        let id = inner.next_id;
        inner.next_id += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: "tools/call".into(),
            params: Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })),
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        inner.pending.push((id, resp_tx));

        let tx = inner.stdin_tx.as_ref().context("no stdin channel")?;
        let msg = serde_json::to_string(&request)?;
        tx.send(msg).await.map_err(|_| anyhow::anyhow!("channel closed"))?;
        drop(inner);

        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            resp_rx,
        ).await.context("Timeout waiting for tool response")??;

        if let Some(err) = resp.error {
            anyhow::bail!("Lightpanda tool error: {}", err.message);
        }

        let result = resp.result.unwrap_or(serde_json::Value::Null);
        let tool_result: McpToolResult = serde_json::from_value(result.clone())
            .unwrap_or(McpToolResult {
                content: vec![McpContent {
                    content_type: "text".into(),
                    text: Some(serde_json::to_string_pretty(&result).unwrap_or_default()),
                }],
                is_error: false,
            });

        Ok(tool_result)
    }

    /// Stop the subprocess.
    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(mut child) = inner.child.take() {
            let _ = child.kill().await;
        }
        inner.stdin_tx = None;
        inner.initialized = false;
        inner.pending.clear();
    }
}

/// Extract text from an MCP tool result.
pub fn extract_text(result: &McpToolResult) -> String {
    result.content.iter()
        .filter_map(|c| c.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n")
}
