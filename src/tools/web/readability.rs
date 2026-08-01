//! Readability extractor ported from CRW (fastCRW).
//! Uses text-density scoring and CSS selector drill-down to extract main content.

// ─── Readability Extractor (CRW-inspired) ─────────────────────

/// Compute text-to-HTML ratio as a content density signal.
/// Returns a value in [0, 1]: higher = more text relative to markup.
/// Ported from CRW's text_density function.
pub fn text_density(html: &str) -> f64 {
    let doc = scraper::Html::parse_fragment(html);
    let text_len: usize = doc.root_element().text().map(|t| t.len()).sum();
    if html.is_empty() {
        return 0.0;
    }
    text_len as f64 / html.len() as f64
}

/// Scored selectors for main content extraction, ordered by priority.
/// Copied from CRW's readability.rs selector list.
pub const SCORED_SELECTORS: &[&str] = &[
    // Priority: semantic HTML first
    "article",
    "main",
    "[role='main']",
    // Content containers
    ".post-content",
    ".article-body",
    ".entry-content",
    ".article-content",
    ".post-body",
    ".story-body",
    ".content-body",
    "#main-content",
    "#article",
    "#content",
    ".content",
    ".main",
    "[itemprop='articleBody']",
    "[itemprop='text']",
    // MDN
    ".main-page-content",
    // StackOverflow
    ".js-post-body",
    ".s-prose",
    "#question",
    // Generic
    ".page-content",
    "#page-content",
    "[role='article']",
    // Wikipedia / MediaWiki
    ".mw-parser-output",
    "#mw-content-text",
    "#bodyContent",
    ".mw-body-content",
];

/// Inner selectors for drill-down when a priority selector is too broad (>90% of body).
pub const DRILL_DOWN_SELECTORS: &[&str] = &[
    ".main-page-content",
    ".article-content",
    ".post-content",
    ".entry-content",
    ".content-body",
    ".article-body",
    "[itemprop='articleBody']",
    "[itemprop='text']",
    ".mw-parser-output",
    "#mw-content-text",
    "#content",
    ".content",
    "article",
];

/// When a priority selector is "too broad" (>90% of body), drill down into it
/// to find a narrower content element. Ported from CRW's find_content_within.
pub fn find_content_within(parent_el: &scraper::ElementRef, parent_len: usize) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    for sel_str in DRILL_DOWN_SELECTORS {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            for el in parent_el.select(&sel) {
                let content = el.html();
                if content.len() < 200 {
                    continue;
                }
                // Skip if still too broad relative to parent
                if content.len() as f64 / parent_len as f64 > 0.85 {
                    continue;
                }
                let score = text_density(&content) * (content.len() as f64).ln();
                if best.as_ref().is_none_or(|(_, s)| score > *s) {
                    best = Some((content, score));
                }
            }
        }
    }
    best.map(|(c, _)| c)
}

/// Extract the main content from HTML using text-density scoring.
/// Ported from CRW's extract_main_content with drill-down logic.
/// Falls back to body if no scored candidate is found.
pub fn extract_main_content(html: &str) -> String {
    let document = scraper::Html::parse_document(html);

    // Compute body length once for ratio checks
    let body_len = scraper::Selector::parse("body")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .map(|b| b.html().len())
        .unwrap_or(html.len());

    // Score all candidate selectors by text density and pick the best
    let mut best: Option<(String, f64)> = None;
    for sel_str in SCORED_SELECTORS {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            for el in document.select(&sel) {
                let content = el.html();
                if content.len() < 100 {
                    continue;
                }
                // Skip selectors that wrap nearly the entire body
                if body_len > 0 && content.len() as f64 / body_len as f64 > 0.9 {
                    if let Some(narrowed) = find_content_within(&el, content.len()) {
                        return narrowed;
                    }
                    continue;
                }
                let score = text_density(&content) * (content.len() as f64).ln();
                if best.as_ref().is_none_or(|(_, s)| score > *s) {
                    best = Some((content, score));
                }
            }
        }
    }

    if let Some((content, _)) = best {
        return content;
    }

    // Last resort: return full body
    if let Ok(sel) = scraper::Selector::parse("body") {
        if let Some(body) = document.select(&sel).next() {
            return body.inner_html();
        }
    }

    html.to_string()
}

/// Extract a semantic excerpt from a page.
/// Uses CRW-inspired text-density scoring with drill-down logic.
pub fn extract_semantic_excerpt(html: &str, _title: &str, max_chars: usize) -> String {
    let main_html = extract_main_content(html);
    let doc = scraper::Html::parse_fragment(&main_html);
    let text: String = doc.root_element().text().collect::<Vec<_>>().join(" ");
    let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned.chars().take(max_chars).collect()
}

/// Extract the real URL from a DuckDuckGo redirect link.
/// DuckDuckGo wraps all result links in /l/?uddg=<encoded_url>&rut=...
/// Works for both relative (/l/?uddg=...) and absolute URLs.
pub fn extract_ddg_redirect_url(href: &str) -> Option<String> {
    let pos = href.find("uddg=")?;
    let encoded = &href[pos + 5..];
    let end = encoded.find('&').unwrap_or(encoded.len());
    url::form_urlencoded::parse(format!("x={}", &encoded[..end]).as_bytes())
        .find(|(k, _)| k == "x")
        .map(|(_, v)| v.to_string())
}
