use crate::tools::types::*;
use std::sync::Arc;
use tokio::sync::Mutex;

use std::sync::OnceLock;

static CURRENT_URL: OnceLock<Mutex<String>> = OnceLock::new();

fn current_url() -> String {
    CURRENT_URL
        .get_or_init(|| Mutex::new("about:blank".to_string()))
        .try_lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "about:blank".to_string())
}

async fn set_current_url(url: &str) {
    if let Some(m) = CURRENT_URL.get() {
        if let Ok(mut guard) = m.try_lock() {
            *guard = url.to_string();
        }
    }
}

async fn run_obscura_cli(args: &[&str], stdin_js: Option<&str>) -> Result<LpToolOutput, String> {
    let mut cmd = tokio::process::Command::new("obscura");
    cmd.arg("fetch");
    for arg in args {
        cmd.arg(arg);
    }
    if let Some(js) = stdin_js {
        cmd.arg("--eval").arg(js);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute obscura: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Ok(LpToolOutput {
            success: false,
            content: if stderr.is_empty() { stdout } else { stderr },
            meta: BrowserMeta {
                url: String::new(),
                title: None,
                operation: "obscura_cli".to_string(),
                elapsed_ms: 0,
            },
        });
    }

    Ok(LpToolOutput {
        success: true,
        content: stdout,
        meta: BrowserMeta {
            url: String::new(),
            title: None,
            operation: "obscura_cli".to_string(),
            elapsed_ms: 0,
        },
    })
}

pub async fn lp_goto(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpGotoInput,
) -> Result<LpToolOutput, String> {
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let args = vec![input.url.as_str(), "--wait-until", wait_until, "--stealth"];

    let mut output = run_obscura_cli(&args, None).await?;
    output.meta.url = input.url.clone();
    output.meta.operation = "goto".to_string();
    set_current_url(&input.url).await;
    Ok(output)
}

pub async fn lp_markdown(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpMarkdownInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let mut args = vec![url.as_str(), "--dump", "markdown", "--stealth"];
    if let Some(ref sm) = input.strip_mode {
        args.push("--strip-mode");
        args.push(sm);
    }
    let mut output = run_obscura_cli(&args, None).await?;
    output.meta.url = url;
    output.meta.operation = "markdown".to_string();
    Ok(output)
}

pub async fn lp_links(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    _input: LpLinksInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let args = vec![url.as_str(), "--dump", "links", "--stealth"];
    let mut output = run_obscura_cli(&args, None).await?;
    output.meta.url = url;
    output.meta.operation = "links".to_string();
    Ok(output)
}

pub async fn lp_evaluate(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpEvaluateInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&input.expression)).await?;
    output.meta.url = url;
    output.meta.operation = "evaluate".to_string();
    Ok(output)
}

pub async fn lp_semantic_tree(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    _input: LpSemanticTreeInput,
) -> Result<LpToolOutput, String> {
    Ok(LpToolOutput {
        success: false,
        content: "Obscura CLI does not support semantic_tree. Use evaluate with custom JS or call obscura directly with --dump semantic_tree.".to_string(),
        meta: BrowserMeta {
            url: String::new(),
            title: None,
            operation: "semantic_tree".to_string(),
            elapsed_ms: 0,
        },
    })
}

pub async fn lp_structured_data(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    _input: LpStructuredDataInput,
) -> Result<LpToolOutput, String> {
    Ok(LpToolOutput {
        success: false,
        content: "Obscura CLI does not support structured_data extraction. Use evaluate with custom JS to extract JSON-LD, OpenGraph, or microdata.".to_string(),
        meta: BrowserMeta {
            url: String::new(),
            title: None,
            operation: "structured_data".to_string(),
            elapsed_ms: 0,
        },
    })
}

pub async fn lp_detect_forms(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    _input: LpDetectFormsInput,
) -> Result<LpToolOutput, String> {
    Ok(LpToolOutput {
        success: false,
        content: "Obscura CLI does not support detect_forms. Use evaluate with custom JS to enumerate form elements.".to_string(),
        meta: BrowserMeta {
            url: String::new(),
            title: None,
            operation: "detect_forms".to_string(),
            elapsed_ms: 0,
        },
    })
}

pub async fn lp_click(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpClickInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let js = format!(
        "document.querySelector('{}')?.click(); 'clicked'",
        input.selector.replace('\'', "\\'")
    );
    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&js)).await?;
    output.meta.url = url;
    output.meta.operation = "click".to_string();
    Ok(output)
}

pub async fn lp_fill(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpFillInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let js = format!(
        "const el = document.querySelector('{}'); if(el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles:true}})); }} 'filled'",
        input.selector.replace('\'', "\\'"),
        input.value.replace('\'', "\\'")
    );
    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&js)).await?;
    output.meta.url = url;
    output.meta.operation = "fill".to_string();
    Ok(output)
}

pub async fn lp_scroll(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpScrollInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let direction = input.direction.as_deref().unwrap_or("down");
    let pixels = input.pixels.unwrap_or(500);

    let js = match direction {
        "up" => format!("window.scrollBy(0, -{}); 'scrolled'", pixels),
        "down" => format!("window.scrollBy(0, {}); 'scrolled'", pixels),
        "left" => format!("window.scrollBy(-{}, 0); 'scrolled'", pixels),
        "right" => format!("window.scrollBy({}, 0); 'scrolled'", pixels),
        _ => format!("window.scrollBy(0, {}); 'scrolled'", pixels),
    };

    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&js)).await?;
    output.meta.url = url;
    output.meta.operation = "scroll".to_string();
    Ok(output)
}

pub async fn lp_wait_for_selector(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    input: LpWaitForSelectorInput,
) -> Result<LpToolOutput, String> {
    let url = current_url();
    let args = vec![
        url.as_str(),
        "--stealth",
        "--wait-selector",
        &input.selector,
    ];
    let mut output = run_obscura_cli(&args, None).await?;
    output.meta.url = url;
    output.meta.operation = "wait_for_selector".to_string();
    Ok(output)
}

pub async fn lp_interactive_elements(
    _manager: &Arc<Mutex<Option<crate::obscura::ObscuraManager>>>,
    _input: LpInteractiveElementsInput,
) -> Result<LpToolOutput, String> {
    Ok(LpToolOutput {
        success: false,
        content: "Obscura CLI does not support interactive_elements detection. Use evaluate with custom JS to find buttons, links, and inputs.".to_string(),
        meta: BrowserMeta {
            url: String::new(),
            title: None,
            operation: "interactive_elements".to_string(),
            elapsed_ms: 0,
        },
    })
}
