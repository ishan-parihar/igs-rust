use crate::lightpanda_mcp::{self, LightpandaMcpClient};
use crate::tools::types::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Execute a Lightpanda MCP tool call and return the text content.
async fn call_lp_tool(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<LpToolOutput, String> {
    let mut guard = client.lock().await;
    if guard.is_none() {
        *guard = Some(LightpandaMcpClient::new(binary_path.clone()));
    }
    let lp = guard.as_ref().unwrap();

    match lp.call_tool(tool_name, arguments).await {
        Ok(result) => {
            let text = lightpanda_mcp::extract_text(&result);
            Ok(LpToolOutput {
                success: !result.is_error,
                content: text,
                meta: serde_json::json!({ "tool": tool_name }),
            })
        }
        Err(e) => Ok(LpToolOutput {
            success: false,
            content: format!("Lightpanda MCP error: {}", e),
            meta: serde_json::json!({ "tool": tool_name, "error": true }),
        }),
    }
}

// ─── Tool Implementations ──────────────────────────────────────

pub async fn lp_goto(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpGotoInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({ "url": input.url });
    if let Some(wu) = input.wait_until {
        args["waitUntil"] = serde_json::json!(wu);
    }
    call_lp_tool(client, binary_path, "goto", args).await
}

pub async fn lp_markdown(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpMarkdownInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(sm) = input.strip_mode {
        args["stripMode"] = serde_json::json!(sm);
    }
    call_lp_tool(client, binary_path, "markdown", args).await
}

pub async fn lp_links(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpLinksInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(sel) = input.selector {
        args["selector"] = serde_json::json!(sel);
    }
    call_lp_tool(client, binary_path, "links", args).await
}

pub async fn lp_evaluate(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpEvaluateInput,
) -> Result<LpToolOutput, String> {
    call_lp_tool(client, binary_path, "evaluate", serde_json::json!({
        "expression": input.expression
    })).await
}

pub async fn lp_semantic_tree(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpSemanticTreeInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(it) = input.include_text {
        args["includeText"] = serde_json::json!(it);
    }
    call_lp_tool(client, binary_path, "semantic_tree", args).await
}

pub async fn lp_structured_data(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpStructuredDataInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(v) = input.jsonld { args["jsonld"] = serde_json::json!(v); }
    if let Some(v) = input.opengraph { args["opengraph"] = serde_json::json!(v); }
    if let Some(v) = input.microdata { args["microdata"] = serde_json::json!(v); }
    call_lp_tool(client, binary_path, "structuredData", args).await
}

pub async fn lp_detect_forms(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpDetectFormsInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(sel) = input.selector {
        args["selector"] = serde_json::json!(sel);
    }
    call_lp_tool(client, binary_path, "detectForms", args).await
}

pub async fn lp_click(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpClickInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({ "selector": input.selector });
    if let Some(wfn) = input.wait_for_navigation {
        args["waitForNavigation"] = serde_json::json!(wfn);
    }
    call_lp_tool(client, binary_path, "click", args).await
}

pub async fn lp_fill(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpFillInput,
) -> Result<LpToolOutput, String> {
    call_lp_tool(client, binary_path, "fill", serde_json::json!({
        "selector": input.selector,
        "value": input.value,
    })).await
}

pub async fn lp_scroll(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpScrollInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(d) = input.direction { args["direction"] = serde_json::json!(d); }
    if let Some(p) = input.pixels { args["pixels"] = serde_json::json!(p); }
    call_lp_tool(client, binary_path, "scroll", args).await
}

pub async fn lp_wait_for_selector(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpWaitForSelectorInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({ "selector": input.selector });
    if let Some(t) = input.timeout_ms { args["timeout"] = serde_json::json!(t); }
    call_lp_tool(client, binary_path, "waitForSelector", args).await
}

pub async fn lp_interactive_elements(
    client: &Arc<Mutex<Option<LightpandaMcpClient>>>,
    binary_path: &std::path::PathBuf,
    input: LpInteractiveElementsInput,
) -> Result<LpToolOutput, String> {
    let mut args = serde_json::json!({});
    if let Some(sel) = input.selector { args["selector"] = serde_json::json!(sel); }
    call_lp_tool(client, binary_path, "interactiveElements", args).await
}
