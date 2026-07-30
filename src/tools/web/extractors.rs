//! Web extraction tools: scrape, crawl, extract, and map.
//! Handles multi-provider scraping (default HTTP, Lightpanda, Obscura),
//! BFS crawling, structured content extraction, and sitemap discovery.

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types::*;
use super::scoring::{extract_internal_links, bm25_score_chunks, BM25_MIN_THRESHOLD};
use std::collections::HashMap;



pub async fn web_scrape(input: WebScrapeInput, http: &HttpClient, settings: &crate::types::Settings) -> Result<WebScrapeOutput, String> {

    // Determine provider: explicit input, or browser.default from settings
    let provider = input.provider.as_deref().unwrap_or(&settings.browser.default);

    match provider {
        "lightpanda" => web_scrape_lightpanda(&input, settings).await,
        "obscura" => web_scrape_obscura(&input, settings).await,
        _ => web_scrape_default(&input, http, settings).await,
    }
}

/// Scrape using plain HTTP + html-to-markdown-rs (default provider)
pub(super) async fn web_scrape_default(
    input: &WebScrapeInput,
    http: &HttpClient,
    _settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {

    let body = match http.fetch(&input.url, None, "bypass").await {
        Ok(outcome) => {
            let http_mod::FetchOutcome::Response(resp, _, _) = outcome else {
                unreachable!("bypass cache mode never returns Cached")
            };
            if resp.status < 200 || resp.status >= 400 {
                return Err(format!("HTTP {} for URL: {}", resp.status, input.url));
            }
            resp.body_text
        }
        Err(e) => return Err(format!("Scrape failed: {}", e)),
    };

    extract_scrape_output(&input.url, &body, "default", input.formats.as_deref())
}

/// Scrape using Lightpanda headless browser (JS rendering)
pub(super) async fn web_scrape_lightpanda(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    let lp_settings = &settings.browser.lightpanda;
    if !lp_settings.enabled {
        return Err(
            "Lightpanda is not enabled. Set browser.lightpanda.enabled=true in settings.yml to use provider='lightpanda'"
                .into(),
        );
    }

    let lightpanda = crate::lightpanda::LightpandaManager::new(lp_settings);
    let obey_robots = lp_settings.obey_robots;
    let dump_format = "markdown";
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");

    let body = lightpanda
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            input.include_frames.unwrap_or(false),
            input.wait_selector.as_deref(),
        )
        .await
        .map_err(|e| format!("Lightpanda scrape failed: {}", e))?;

    extract_scrape_output(&input.url, &body, "lightpanda", input.formats.as_deref())
}

/// Scrape using Obscura headless browser (JS rendering)
pub(super) async fn web_scrape_obscura(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    let obs_settings = &settings.browser.obscura;
    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use provider='obscura'"
                .into(),
        );
    }

    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let obey_robots = obs_settings.obey_robots;
    let dump_format = "markdown";
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");

    let body = obscura
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            input.include_frames.unwrap_or(false),
            input.wait_selector.as_deref(),
        )
        .await
        .map_err(|e| format!("Obscura scrape failed: {}", e))?;

    extract_scrape_output(&input.url, &body, "obscura", input.formats.as_deref())
}

pub(super) fn extract_scrape_output(
    url: &str,
    body: &str,
    _provider: &str,
    _formats: Option<&[String]>,
) -> Result<WebScrapeOutput, String> {
    let doc = scraper::Html::parse_document(body);

    let title = scraper::Selector::parse("title")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty());

    let description = scraper::Selector::parse("meta[name='description']")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.attr("content").map(|s| s.to_string()));

    let og_title = scraper::Selector::parse("meta[property='og:title']")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.attr("content").map(|s| s.to_string()));

    let og_description = scraper::Selector::parse("meta[property='og:description']")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.attr("content").map(|s| s.to_string()));

    let mut headings = Vec::new();
    for tag in &["h1", "h2", "h3"] {
        if let Ok(sel) = scraper::Selector::parse(tag) {
            for el in doc.select(&sel) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    headings.push(text);
                }
            }
        }
    }

    let links_count = scraper::Selector::parse("a[href]")
        .ok()
        .map(|sel| doc.select(&sel).count())
        .unwrap_or(0);

    let markdown = {
        let converted = html_to_markdown_rs::convert(body, None)
            .ok()
            .and_then(|r| r.content)
            .filter(|s: &String| !s.trim().is_empty());
        converted.unwrap_or_else(|| {
            let main_content: String = doc.root_element().text().collect::<String>();
            main_content
                .split_whitespace()
                .take(2000)
                .collect::<Vec<_>>()
                .join(" ")
        })
    };

    Ok(WebScrapeOutput {
        success: true,
        url: url.to_string(),
        title,
        markdown: Some(markdown),
        metadata: Some(ScrapeStructuredData {
            description,
            og_title,
            og_description,
            links_count,
            headings,
        }),
        meta: ScrapeMeta {
            url: url.to_string(),
            status: 200,
            content_type: None,
            elapsed_ms: 0,
            js_rendered: false,
        },
    })
}

pub async fn web_crawl(input: WebCrawlInput, settings: &crate::types::Settings) -> Result<WebCrawlOutput, String> {

    // Use browser.default from settings
    let provider = &settings.browser.default;

    match provider.as_str() {
        "lightpanda" => web_crawl_lightpanda(&input, settings).await,
        "obscura" => web_crawl_obscura(&input, settings).await,
        _ => Err(
            "web.crawl requires a headless browser. Set browser.default to 'lightpanda' or 'obscura' in settings.yml"
                .into(),
        ),
    }
}

pub(super) async fn web_crawl_lightpanda(
    input: &WebCrawlInput,
    settings: &crate::types::Settings,
) -> Result<WebCrawlOutput, String> {
    let lp_settings = &settings.browser.lightpanda;
    if !lp_settings.enabled {
        return Err(
            "Lightpanda is not enabled. Set browser.lightpanda.enabled=true in settings.yml"
                .into(),
        );
    }

    let lightpanda = crate::lightpanda::LightpandaManager::new(lp_settings);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);
    let obey_robots = input.obey_robots.unwrap_or(lp_settings.obey_robots);
    let dump_format = input.dump_format.as_deref().unwrap_or("markdown");
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let include_frames = input.include_frames.unwrap_or(false);
    let wait_selector = input.wait_selector.as_deref();

    let content = lightpanda
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            include_frames,
            wait_selector,
        )
        .await
        .map_err(|e| format!("Lightpanda fetch failed: {}", e))?;

    let title = {
        let doc = scraper::Html::parse_document(&content);
        scraper::Selector::parse("title")
            .ok()
            .and_then(|sel| doc.select(&sel).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
    };

    let mut pages = vec![CrawledPage {
        url: input.url.clone(),
        title,
        content,
        depth: 0,
        status: "ok".to_string(),
    }];

    if max_depth > 0 {
        let base_url = url::Url::parse(&input.url)
            .map_err(|e| format!("Invalid URL '{}': {}", input.url, e))?;
        let base_host = base_url.host_str().unwrap_or("").to_string();

        let mut queue: std::collections::VecDeque<(String, i32)> =
            std::collections::VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        visited.insert(input.url.clone());

        {
            let doc = scraper::Html::parse_document(&pages[0].content);
            let sel = scraper::Selector::parse("a[href]").expect("valid selector");
            for url_str in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                if !visited.contains(&url_str) {
                    visited.insert(url_str.clone());
                    queue.push_back((url_str, 1));
                }
            }
        }

        while let Some((url_str, depth)) = queue.pop_front() {
            if pages.len() >= max_pages as usize {
                break;
            }

            match lightpanda
                .fetch_with_all_options(
                    &url_str,
                    dump_format,
                    obey_robots,
                    wait_until,
                    include_frames,
                    wait_selector,
                )
                .await
            {
                Ok(content) => {
                    let title = {
                        let doc = scraper::Html::parse_document(&content);
                        scraper::Selector::parse("title")
                            .ok()
                            .and_then(|sel| doc.select(&sel).next())
                            .map(|el| el.text().collect::<String>().trim().to_string())
                    };

                    if depth < max_depth {
                        let doc = scraper::Html::parse_document(&content);
                        let sel = scraper::Selector::parse("a[href]").expect("valid selector");
                        for link_url in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                            if !visited.contains(&link_url)
                                && pages.len() + queue.len() < max_pages as usize
                            {
                                visited.insert(link_url.clone());
                                queue.push_back((link_url, depth + 1));
                            }
                        }
                    }

                    pages.push(CrawledPage {
                        url: url_str,
                        title,
                        content,
                        depth,
                        status: "ok".to_string(),
                    });
                }
                Err(e) => {
                    pages.push(CrawledPage {
                        url: url_str,
                        title: None,
                        content: format!("Error: {}", e),
                        depth,
                        status: "error".to_string(),
                    });
                }
            }
        }
    }

    let count = pages.len();
    Ok(WebCrawlOutput {
        success: true,
        start_url: input.url.clone(),
        pages,
        count,
        meta: WebCrawlMeta {
            provider: "lightpanda".to_string(),
            max_depth,
            max_pages,
            obey_robots,
            dump_format: dump_format.to_string(),
            wait_until: wait_until.to_string(),
            include_frames,
        },
    })
}

pub(super) async fn web_crawl_obscura(
    input: &WebCrawlInput,
    settings: &crate::types::Settings,
) -> Result<WebCrawlOutput, String> {
    let obs_settings = &settings.browser.obscura;
    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use web.crawl"
                .into(),
        );
    }

    let obscura = crate::obscura::ObscuraManager::new(obs_settings);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);
    let obey_robots = input.obey_robots.unwrap_or(obs_settings.obey_robots);
    let dump_format = input.dump_format.as_deref().unwrap_or("markdown");
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let include_frames = input.include_frames.unwrap_or(false);
    let wait_selector = input.wait_selector.as_deref();

    let content = obscura
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            include_frames,
            wait_selector,
        )
        .await
        .map_err(|e| format!("Obscura fetch failed: {}", e))?;

    let title = {
        let doc = scraper::Html::parse_document(&content);
        scraper::Selector::parse("title")
            .ok()
            .and_then(|sel| doc.select(&sel).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
    };

    let mut pages = vec![CrawledPage {
        url: input.url.clone(),
        title,
        content,
        depth: 0,
        status: "ok".to_string(),
    }];

    if max_depth > 0 {
        let base_url = url::Url::parse(&input.url)
            .map_err(|e| format!("Invalid URL '{}': {}", input.url, e))?;
        let base_host = base_url.host_str().unwrap_or("").to_string();

        let mut queue: std::collections::VecDeque<(String, i32)> =
            std::collections::VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        visited.insert(input.url.clone());

        {
            let doc = scraper::Html::parse_document(&pages[0].content);
            let sel = scraper::Selector::parse("a[href]").expect("valid selector");
            for url_str in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                if !visited.contains(&url_str) {
                    visited.insert(url_str.clone());
                    queue.push_back((url_str, 1));
                }
            }
        }

        while let Some((url_str, depth)) = queue.pop_front() {
            if pages.len() >= max_pages as usize {
                break;
            }

            match obscura
                .fetch_with_all_options(
                    &url_str,
                    dump_format,
                    obey_robots,
                    wait_until,
                    include_frames,
                    wait_selector,
                )
                .await
            {
                Ok(content) => {
                    let title = {
                        let doc = scraper::Html::parse_document(&content);
                        scraper::Selector::parse("title")
                            .ok()
                            .and_then(|sel| doc.select(&sel).next())
                            .map(|el| el.text().collect::<String>().trim().to_string())
                    };

                    if depth < max_depth {
                        let doc = scraper::Html::parse_document(&content);
                        let sel = scraper::Selector::parse("a[href]").expect("valid selector");
                        for link_url in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                            if !visited.contains(&link_url)
                                && pages.len() + queue.len() < max_pages as usize
                            {
                                visited.insert(link_url.clone());
                                queue.push_back((link_url, depth + 1));
                            }
                        }
                    }

                    pages.push(CrawledPage {
                        url: url_str,
                        title,
                        content,
                        depth,
                        status: "ok".to_string(),
                    });
                }
                Err(e) => {
                    pages.push(CrawledPage {
                        url: url_str,
                        title: None,
                        content: format!("Error: {}", e),
                        depth,
                        status: "error".to_string(),
                    });
                }
            }
        }
    }

    let count = pages.len();
    Ok(WebCrawlOutput {
        success: true,
        start_url: input.url.clone(),
        pages,
        count,
        meta: WebCrawlMeta {
            provider: "obscura".to_string(),
            max_depth,
            max_pages,
            obey_robots,
            dump_format: dump_format.to_string(),
            wait_until: wait_until.to_string(),
            include_frames,
        },
    })
}

/// Content cleaning selectors (from CRW porting analysis).
/// These selectors target navigation, ads, cookie banners, and boilerplate.
/// Remove boilerplate text patterns from content using word-boundary matching.
/// Uses space-delimited tokenization to avoid corrupting words like "cookies" → "".
pub(super) fn remove_boilerplate(text: &str) -> String {
    let boilerplate_phrases = [
        "privacy policy", "terms of service", "all rights reserved",
        "subscribe to", "sign up for", "newsletter",
        "follow us on", "share this", "tweet this",
        "sponsored content", "loading...", "click here to", "read more:",
    ];
    let boilerplate_words = ["cookie", "copyright", "advertisement"];
    let mut result = text.to_string();
    // Replace full phrases first
    for phrase in &boilerplate_phrases {
        result = result.replace(phrase, "");
    }
    // Replace single words with word-boundary check
    for word in &boilerplate_words {
        // Match word with space before or after, or at start/end
        let mut new_result = String::with_capacity(result.len());
        for token in result.split(' ') {
            if token.eq_ignore_ascii_case(word) {
                continue; // skip the word
            }
            if !new_result.is_empty() {
                new_result.push(' ');
            }
            new_result.push_str(token);
        }
        result = new_result;
    }
    // Collapse multiple spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}

/// Detect content type from page structure and metadata.
pub(super) fn detect_content_type(doc: &scraper::Html) -> Option<String> {
    // Check JSON-LD type
    if let Ok(sel) = scraper::Selector::parse("script[type='application/ld+json']") {
        for el in doc.select(&sel) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&el.text().collect::<String>()) {
                if let Some(t) = json["@type"].as_str() {
                    return Some(t.to_string());
                }
            }
        }
    }

    // Check OG type
    if let Ok(sel) = scraper::Selector::parse("meta[property='og:type']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(t) = el.attr("content") {
                return Some(t.to_string());
            }
        }
    }

    None // Only expose structured types from JSON-LD / OG; avoid fragile title heuristics
}

/// Detect language from meta tags.
pub(super) fn detect_language(doc: &scraper::Html) -> Option<String> {
    // Check meta lang attribute
    if let Ok(sel) = scraper::Selector::parse("html[lang]") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.attr("lang") {
                return Some(lang.to_string());
            }
        }
    }
    // Check meta http-equiv
    if let Ok(sel) = scraper::Selector::parse("meta[http-equiv='content-language']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.attr("content") {
                return Some(lang.to_string());
            }
        }
    }
    // Check og:locale
    if let Ok(sel) = scraper::Selector::parse("meta[property='og:locale']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.attr("content") {
                return Some(lang.split('_').next().unwrap_or(lang).to_string());
            }
        }
    }
    None
}

/// Extract structured content from one or more URLs using Obscura.
/// Single-URL mode: extracts content from `input.url`.
/// Batch mode: if `input.urls` has multiple entries, processes them in parallel
/// (capped at 5 concurrent extractions) and returns the first successful result.
pub async fn web_extract(input: WebExtractInput, settings: &crate::types::Settings) -> Result<WebExtractOutput, String> {
    let obs_settings = &settings.browser.obscura;

    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use web.extract"
                .into(),
        );
    }

    // Determine URLs to process
    if let Some(ref batch_urls) = input.urls {
        if batch_urls.len() > 1 {
            // Batch mode: parallel extraction with concurrency cap
            let urls_clone = batch_urls.clone();
            return web_extract_batch(input, &urls_clone, settings).await;
        }
    }

    extract_single_url(&input.url, &input, settings).await
}

/// Batch-extract multiple URLs in parallel (max 5 concurrent).
async fn web_extract_batch(
    input: WebExtractInput,
    urls: &[String],
    settings: &crate::types::Settings,
) -> Result<WebExtractOutput, String> {
    let start = std::time::Instant::now();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(5));

    let mut handles = Vec::with_capacity(urls.len());
    for url in urls {
        let url = url.clone();
        let settings = settings.clone();
        let sem = semaphore.clone();
        let clean_content = input.clean_content;
        let structured_data = input.structured_data;
        let extract_links = input.extract_links;
        let extract_images = input.extract_images;
        let include_html = input.include_html;
        let query = input.query.clone();
        let selectors = input.selectors.clone();
        let wait_selector = input.wait_selector.clone();
        let output_schema = input.output_schema.clone();
        let extract_prompt = input.extract_prompt.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let batch_input = WebExtractInput {
                url,
                urls: None,
                selectors,
                structured_data,
                extract_links,
                extract_images,
                wait_selector,
                include_html,
                clean_content: clean_content,
                query,
                output_schema: output_schema.clone(),
                extract_prompt: extract_prompt.clone(),
                output: crate::tools::types_base::OutputOptions { format: None },
            };
            extract_single_url(&batch_input.url, &batch_input, &settings).await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => results.push(result),
            Ok(Err(e)) => {
                // Log error but continue with other results
                tracing::warn!("Batch extract error: {}", e);
            }
            Err(e) => {
                tracing::warn!("Batch extract task panicked: {}", e);
            }
        }
    }

    if results.is_empty() {
        return Err("All batch extractions failed".into());
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let count = results.len();

    // Return first successful result with batch metadata
    let mut first = results.into_iter().next().unwrap();
    first.meta.elapsed_ms = elapsed_ms;
    first.meta.provider = format!("obscura (batch: {} urls, {} succeeded)", urls.len(), count);
    Ok(first)
}

/// Extract content from a single URL using Obscura.
async fn extract_single_url(
    url: &str,
    input: &WebExtractInput,
    settings: &crate::types::Settings,
) -> Result<WebExtractOutput, String> {
    let start = std::time::Instant::now();
    let obs_settings = &settings.browser.obscura;
    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let wait_until = "networkidle";
    let clean_content = input.clean_content.unwrap_or(false);

    // Fetch the page with Obscura (JS rendering)
    let html = obscura
        .fetch_with_all_options(
            url,
            "html",
            false,
            wait_until,
            false,
            input.wait_selector.as_deref(),
        )
        .await
        .map_err(|e| format!("Obscura fetch failed: {}", e))?;

    let doc = scraper::Html::parse_document(&html);

    // Extract title
    let title = scraper::Selector::parse("title")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty());

    // Extract main text content (with optional cleaning)
    let raw_content = extract_main_text(&doc);
    let content = if clean_content {
        remove_boilerplate(&raw_content)
    } else {
        raw_content
    };

    // If a query is provided, chunk-score the content with BM25 for relevance ranking
    let content = if let Some(ref query) = input.query {
        if !query.trim().is_empty() && !content.is_empty() {
            // Split content into paragraph chunks on double-newlines
            let chunks: Vec<String> = content
                .split("\n\n")
                .map(|s| s.trim().to_string())
                .filter(|s| s.len() > 50)
                .collect();

            if chunks.len() > 3 {
                // BM25-score all chunks, keep only those with meaningful relevance
                let scored = bm25_score_chunks(&chunks, query, chunks.len());
                let filtered: Vec<(usize, &str)> = scored
                    .iter()
                    .filter(|c| c.score > BM25_MIN_THRESHOLD)
                    .map(|c| (c.index, c.content.as_str()))
                    .collect();
                if !filtered.is_empty() {
                    // Reassemble in original reading order
                    let mut ordered = filtered;
                    ordered.sort_by_key(|(i, _)| *i);
                    ordered.into_iter().map(|(_, c)| c).collect::<Vec<_>>().join("\n\n")
                } else {
                    content
                }
            } else {
                content
            }
        } else {
            content
        }
    } else {
        content
    };

    // Generate markdown
    let markdown = html_to_markdown_rs::convert(&html, None)
        .ok()
        .and_then(|r| r.content)
        .filter(|s| !s.trim().is_empty());

    // Extract metadata (with enhancements)
    let mut metadata = extract_page_metadata(&doc);
    metadata.language = detect_language(&doc);
    metadata.content_type = detect_content_type(&doc);
    // reading_time_minutes is already computed in extract_page_metadata

    // Extract structured data if requested
    let structured_data = if input.structured_data.unwrap_or(false) {
        extract_structured_data(&doc)
    } else {
        None
    };

    // Extract links if requested
    let links = if input.extract_links.unwrap_or(false) {
        extract_page_links(&doc)
    } else {
        None
    };

    // Extract images if requested
    let images = if input.extract_images.unwrap_or(false) {
        extract_page_images(&doc)
    } else {
        None
    };

    // Extract elements by selector if provided
    let elements = if let Some(selectors) = &input.selectors {
        extract_by_selectors(&doc, selectors)
    } else {
        None
    };

    // Extract structured data via output_schema (P2.1)
    let _extracted_data = if let Some(ref schema) = input.output_schema {
        Some(extract_by_schema(&doc, schema))
    } else {
        None
    };

    // Include raw HTML if requested
    let html_output = if input.include_html.unwrap_or(false) {
        Some(html.clone())
    } else {
        None
    };

    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(WebExtractOutput {
        success: true,
        url: url.to_string(),
        title,
        content: Some(content),
        markdown,
        html: html_output,
        metadata: Some(metadata),
        structured_data,
        links,
        images,
        elements,
        extracted_data: None,
        meta: ExtractMeta {
            url: url.to_string(),
            provider: "obscura".into(),
            js_rendered: true,
            elapsed_ms,
        },
    })
}

/// Extract main text content from the document body
pub(super) fn extract_main_text(doc: &scraper::Html) -> String {
    // Try to find main content area first
    let main_selectors = ["main", "article", "[role='main']", ".content", "#content"];
    for selector_str in &main_selectors {
        if let Ok(sel) = scraper::Selector::parse(selector_str) {
            if let Some(main_el) = doc.select(&sel).next() {
                let text = main_el.text().collect::<String>();
                let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if cleaned.len() > 100 {
                    return cleaned;
                }
            }
        }
    }
    // Fallback to body
    if let Ok(sel) = scraper::Selector::parse("body") {
        if let Some(body_el) = doc.select(&sel).next() {
            let text = body_el.text().collect::<String>();
            return text.split_whitespace().collect::<Vec<_>>().join(" ");
        }
    }
    // Last resort: root element
    let text: String = doc.root_element().text().collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract page metadata (description, OpenGraph, author, publish date)
pub(super) fn extract_page_metadata(doc: &scraper::Html) -> ExtractMetadata {
    let description = doc.select(&scraper::Selector::parse("meta[name='description']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let og_title = doc.select(&scraper::Selector::parse("meta[property='og:title']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let og_description = doc.select(&scraper::Selector::parse("meta[property='og:description']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let og_image = doc.select(&scraper::Selector::parse("meta[property='og:image']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let author = doc.select(&scraper::Selector::parse("meta[name='author']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let publish_date = doc.select(&scraper::Selector::parse("meta[property='article:published_time']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .or_else(|| {
            doc.select(&scraper::Selector::parse("meta[name='date']").unwrap())
                .next()
                .and_then(|el| el.attr("content"))
        })
        .map(|s| s.to_string());

    let body_text = doc.root_element().text().collect::<String>();
    let word_count = body_text.split_whitespace().count();

    ExtractMetadata {
        description,
        og_title,
        og_description,
        og_image,
        author,
        publish_date,
        word_count,
        reading_time_minutes: Some((word_count as u32 / 200).max(1)),
        language: None, // populated by caller via detect_language()
        content_type: None, // populated by caller via detect_content_type()
    }
}

/// Extract structured data (JSON-LD and OpenGraph)
pub(super) fn extract_structured_data(doc: &scraper::Html) -> Option<StructuredData> {
    // Extract JSON-LD
    let json_ld: Vec<serde_json::Value> = doc
        .select(&scraper::Selector::parse("script[type='application/ld+json']").unwrap())
        .filter_map(|el| {
            let text = el.text().collect::<String>();
            serde_json::from_str(&text).ok()
        })
        .collect();

    // Extract OpenGraph
    let mut opengraph: HashMap<String, String> = HashMap::new();
    for prop in &["og:title", "og:description", "og:image", "og:url", "og:type", "og:site_name"] {
        if let Ok(sel) = scraper::Selector::parse(&format!("meta[property='{}']", prop)) {
            if let Some(el) = doc.select(&sel).next() {
                if let Some(content) = el.attr("content") {
                    opengraph.insert(prop.to_string(), content.to_string());
                }
            }
        }
    }

    if json_ld.is_empty() && opengraph.is_empty() {
        None
    } else {
        Some(StructuredData {
            json_ld: if json_ld.is_empty() { None } else { Some(json_ld) },
            opengraph: if opengraph.is_empty() { None } else { Some(opengraph) },
        })
    }
}

/// Extract all links from the page
pub(super) fn extract_page_links(doc: &scraper::Html) -> Option<Vec<ExtractedLink>> {
    let mut links = Vec::new();
    if let Ok(sel) = scraper::Selector::parse("a[href]") {
        for el in doc.select(&sel) {
            if let Some(href) = el.attr("href") {
                let text = el.text().collect::<String>().trim().to_string();
                let rel = el.attr("rel").map(|s| s.to_string());
                links.push(ExtractedLink {
                    url: href.to_string(),
                    text,
                    rel,
                });
            }
        }
    }
    if links.is_empty() { None } else { Some(links) }
}

/// Extract all images from the page
pub(super) fn extract_page_images(doc: &scraper::Html) -> Option<Vec<ExtractedImage>> {
    let mut images = Vec::new();
    if let Ok(sel) = scraper::Selector::parse("img[src]") {
        for el in doc.select(&sel) {
            if let Some(src) = el.attr("src") {
                let alt = el.attr("alt").map(|s| s.to_string());
                let width = el.attr("width").map(|s| s.to_string());
                let height = el.attr("height").map(|s| s.to_string());
                images.push(ExtractedImage {
                    url: src.to_string(),
                    alt,
                    width,
                    height,
                });
            }
        }
    }
    if images.is_empty() { None } else { Some(images) }
}

/// Extract elements by CSS selectors
pub(super) fn extract_by_selectors(doc: &scraper::Html, selectors: &[String]) -> Option<Vec<ExtractedElement>> {
    let mut elements = Vec::new();
    for selector_str in selectors {
        if let Ok(sel) = scraper::Selector::parse(selector_str) {
            for el in doc.select(&sel) {
                let html = el.html();
                let text = el.text().collect::<String>().trim().to_string();
                elements.push(ExtractedElement {
                    selector: selector_str.clone(),
                    html,
                    text,
                });
            }
        }
    }
    if elements.is_empty() { None } else { Some(elements) }
}

/// Extract structured data by matching a JSON schema to CSS selectors.
/// The schema can have two forms:
/// 1. Object with field names as keys and CSS selectors as string values:
///    `{"title": "h1", "description": "meta[name='description']"}`
/// 2. Object with field names as keys and objects with `selector` + `attr` + `mode`:
///    `{"title": {"selector": "h1", "attr": "text"}}`
/// Modes: "text" (default, inner text), "html" (inner HTML), "attr" (attribute value)
pub(super) fn extract_by_schema(doc: &scraper::Html, schema: &serde_json::Value) -> serde_json::Value {
    let Some(obj) = schema.as_object() else {
        return serde_json::json!({});
    };

    let mut result = serde_json::Map::new();

    for (field_name, field_spec) in obj {
        match field_spec {
            // Simple form: {"title": "h1"} — CSS selector as string value
            serde_json::Value::String(selector_str) => {
                if let Ok(sel) = scraper::Selector::parse(selector_str) {
                    let values: Vec<String> = doc.select(&sel)
                        .map(|el| el.text().collect::<String>().trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if values.len() == 1 {
                        result.insert(field_name.clone(), serde_json::json!(values[0]));
                    } else if values.len() > 1 {
                        result.insert(field_name.clone(), serde_json::json!(values));
                    } else {
                        result.insert(field_name.clone(), serde_json::Value::Null);
                    }
                } else {
                    result.insert(field_name.clone(), serde_json::Value::Null);
                }
            }
            // Detailed form: {"title": {"selector": "h1", "attr": "href", "mode": "attr"}}
            serde_json::Value::Object(spec) => {
                let selector_str = spec.get("selector").and_then(|v| v.as_str()).unwrap_or("");
                let attr = spec.get("attr").and_then(|v| v.as_str());
                let mode = spec.get("mode").and_then(|v| v.as_str()).unwrap_or("text");
                let multi = spec.get("multi").and_then(|v| v.as_bool()).unwrap_or(false);

                if selector_str.is_empty() {
                    result.insert(field_name.clone(), serde_json::Value::Null);
                    continue;
                }

                if let Ok(sel) = scraper::Selector::parse(selector_str) {
                    let values: Vec<String> = doc.select(&sel)
                        .filter_map(|el| {
                            match mode {
                                "html" => Some(el.html()),
                                "attr" => attr.and_then(|a| el.attr(a)).map(|s| s.to_string()),
                                _ => Some(el.text().collect::<String>().trim().to_string()),
                            }
                        })
                        .filter(|s| !s.is_empty())
                        .collect();

                    if multi {
                        result.insert(field_name.clone(), serde_json::json!(values));
                    } else if let Some(first) = values.into_iter().next() {
                        result.insert(field_name.clone(), serde_json::json!(first));
                    } else {
                        result.insert(field_name.clone(), serde_json::Value::Null);
                    }
                } else {
                    result.insert(field_name.clone(), serde_json::Value::Null);
                }
            }
            _ => {
                result.insert(field_name.clone(), field_spec.clone());
            }
        }
    }

    serde_json::Value::Object(result)
}

/// Discover URLs on a website by analyzing sitemap and links.
pub async fn web_map(input: WebMapInput, http: &HttpClient, settings: &crate::types::Settings) -> Result<WebMapOutput, String> {
    let _cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());

    let base_url = input.url.trim_end_matches('/');
    let sitemap_url = format!("{}/sitemap.xml", base_url);

    let mut links: Vec<WebMapLink> = Vec::new();
    let mut sitemap_fetched = false;

    // Try sitemap.xml
    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) =
        http.fetch(&sitemap_url, None, "bypass").await
    {
        // Only treat as fetched if the response is a real sitemap (HTTP 200
        // and body contains <urlset> or <sitemapindex>). A 404 or HTML error
        // page would otherwise silently produce an empty-but-"success" result.
        if resp.status >= 200 && resp.status < 400 {
            let body = &resp.body_text;
            if body.contains("<urlset") || body.contains("<sitemapindex") {
                sitemap_fetched = true;
                let doc = scraper::Html::parse_document(body);
                if let Ok(sel) = scraper::Selector::parse("url") {
                    for el in doc.select(&sel) {
                        if let Ok(loc_sel) = scraper::Selector::parse("loc") {
                            if let Some(loc) = el.select(&loc_sel).next() {
                                let url_str = loc.text().collect::<String>().trim().to_string();
                                if !url_str.is_empty() && !links.iter().any(|l| l.url == url_str) {
                                    let title = scraper::Selector::parse("news\\:title")
                                        .or_else(|_| scraper::Selector::parse("title"))
                                        .ok()
                                        .and_then(|ts| el.select(&ts).next())
                                        .map(|t| t.text().collect::<String>());
                                    links.push(WebMapLink {
                                        url: url_str,
                                        title,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Filter by search term if provided
    if let Some(ref search) = input.search {
        let search_lower = search.to_lowercase();
        links.retain(|link| {
            link.url.to_lowercase().contains(&search_lower)
                || link
                    .title
                    .as_ref()
                    .is_some_and(|t| t.to_lowercase().contains(&search_lower))
        });
    }

    let limit = input.limit.unwrap_or(100) as usize;
    links.truncate(limit);
    let count = links.len();

    Ok(WebMapOutput {
        success: sitemap_fetched,
        url: input.url,
        count,
        links,
        meta: WebMapMeta {
            provider: "sitemap-parser".to_string(),
            limit,
        },
    })
}
