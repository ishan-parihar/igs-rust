use crate::tools::types::*;
use tokio::sync::Mutex;

use std::sync::OnceLock;

static CURRENT_URL: OnceLock<Mutex<String>> = OnceLock::new();

/// Get the current browser URL. Awaits the mutex to avoid the silent
/// "about:blank" fallback that the previous `try_lock()` implementation
/// produced under concurrent MCP calls.
async fn current_url() -> String {
    let m = CURRENT_URL.get_or_init(|| Mutex::new("about:blank".to_string()));
    m.lock().await.clone()
}

/// Set the current browser URL. Awaits the mutex.
async fn set_current_url(url: &str) {
    let m = CURRENT_URL.get_or_init(|| Mutex::new("about:blank".to_string()));
    *m.lock().await = url.to_string();
}

/// Escape a string for safe interpolation inside a single-quoted JavaScript
/// string literal. Escapes backslash, single-quote, double-quote, newline,
/// carriage return, tab, backspace, form-feed, and the </script> closing
/// sequence. Without this, a user-supplied CSS selector like `</script><script>`
/// or a value containing `'` could inject arbitrary JS into the headless
/// browser evaluation context.
fn js_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            _ => out.push(c),
        }
    }
    // Defend against `</script>` HTML/JS context confusion when the JS is
    // later embedded in an HTML page by the headless browser.
    out.replace("</script>", "<\\/script>")
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
    input: LpGotoInput,
) -> Result<LpToolOutput, String> {
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let args = vec![input.url.as_str(), "--wait-until", wait_until, "--stealth"];

    let mut output = run_obscura_cli(&args, None).await?;
    output.meta.url = input.url.clone();
    output.meta.operation = "goto".to_string();
    // Only update the session URL on successful fetch. Previously, a failed
    // goto (e.g., DNS error, 5xx) would still set current_url, causing
    // subsequent lp_markdown/lp_links calls to re-fetch the failed URL.
    if output.success {
        set_current_url(&input.url).await;
    }
    Ok(output)
}

pub async fn lp_markdown(
    input: LpMarkdownInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
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
    _input: LpLinksInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
    let args = vec![url.as_str(), "--dump", "links", "--stealth"];
    let mut output = run_obscura_cli(&args, None).await?;
    output.meta.url = url;
    output.meta.operation = "links".to_string();
    Ok(output)
}

pub async fn lp_evaluate(
    input: LpEvaluateInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&input.expression)).await?;
    output.meta.url = url;
    output.meta.operation = "evaluate".to_string();
    Ok(output)
}

pub async fn lp_click(
    input: LpClickInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
    let js = format!(
        "document.querySelector('{}')?.click(); 'clicked'",
        js_escape(&input.selector)
    );
    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&js)).await?;
    output.meta.url = url;
    output.meta.operation = "click".to_string();
    Ok(output)
}

pub async fn lp_fill(
    input: LpFillInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
    let js = format!(
        "const el = document.querySelector('{}'); if(el) {{ el.value = '{}'; el.dispatchEvent(new Event('input', {{bubbles:true}})); }} 'filled'",
        js_escape(&input.selector),
        js_escape(&input.value)
    );
    let args = vec![url.as_str(), "--stealth"];
    let mut output = run_obscura_cli(&args, Some(&js)).await?;
    output.meta.url = url;
    output.meta.operation = "fill".to_string();
    Ok(output)
}

pub async fn lp_scroll(
    input: LpScrollInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
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
    input: LpWaitForSelectorInput,
) -> Result<LpToolOutput, String> {
    let url = current_url().await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_escape_handles_single_quote() {
        assert_eq!(js_escape("foo'bar"), "foo\\'bar");
    }

    #[test]
    fn js_escape_handles_double_quote() {
        assert_eq!(js_escape("foo\"bar"), "foo\\\"bar");
    }

    #[test]
    fn js_escape_handles_backslash() {
        assert_eq!(js_escape("foo\\bar"), "foo\\\\bar");
    }

    #[test]
    fn js_escape_handles_newline_and_tab() {
        assert_eq!(js_escape("foo\nbar\tbaz"), "foo\\nbar\\tbaz");
    }

    #[test]
    fn js_escape_handles_script_close_tag() {
        // </script> must not appear literally in the escaped output
        let escaped = js_escape("</script>");
        assert!(!escaped.contains("</script>"));
        assert!(escaped.contains("<\\/script>"));
    }

    #[test]
    fn js_escape_preserves_safe_chars() {
        assert_eq!(js_escape("hello world"), "hello world");
        assert_eq!(js_escape("div#id.class"), "div#id.class");
        assert_eq!(js_escape("input[type='text']"), "input[type=\\'text\\']");
    }

    #[test]
    fn js_escape_empty_string() {
        assert_eq!(js_escape(""), "");
    }
}


