use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types::*;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

mod scoring;
mod readability;
mod engines;
pub(crate) use scoring::*;
pub use readability::*;
pub use engines::*;

pub async fn web_search(input: WebSearchInput) -> Result<WebSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let max_results: usize = input.max_results.unwrap_or(10) as usize;
    let depth = input.depth.as_deref().unwrap_or("fast");
    let topic = input.topic.as_deref().unwrap_or("general");
    let content_length = input.content_length.as_deref().unwrap_or("standard");
    let include_highlights = input.include_highlights.unwrap_or(false);
    let is_deep = depth == "deep";

    // Check cache first (skip for deep mode or explicit engines)
    if !is_deep && input.engines.is_none() {
        if let Some(cached) = cache_get(&input.query, topic, max_results, content_length, include_highlights, input.include_answer.unwrap_or(false)) {
            return Ok(cached);
        }
    }

    // Determine which engines to use
    let engines = input.engines.clone().unwrap_or_else(|| route_engines(topic, &input.query));

    let start = Instant::now();

    // Launch all engine queries in parallel
    let mut handles = Vec::new();

    for engine in &engines {
        match engine.as_str() {
            "duckduckgo" => {
                let q = input.query.clone();
                let obs_settings = settings.browser.obscura.clone();
                let include_answer = input.include_answer.unwrap_or(false);
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                let time_range_clone = input.time_range.clone().unwrap_or_default();
                handles.push(tokio::spawn(async move {
                    search_duckduckgo(&q, max_results * 2, include_answer, &time_range_clone, &obs_settings, &http_clone).await
                }));
            }

            "wikipedia" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                handles.push(tokio::spawn(async move {
                    search_wikipedia(&q, (max_results / 2).max(3), &http_clone).await
                }));
            }
            "github" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                let topic_clone = topic.to_string();
                handles.push(tokio::spawn(async move {
                    search_github(&q, max_results, &http_clone, &topic_clone).await
                }));
            }
            "hackernews" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                let time_range_clone = input.time_range.clone().unwrap_or_default();
                handles.push(tokio::spawn(async move {
                    search_hackernews(&q, max_results, &http_clone, &time_range_clone).await
                }));
            }
            "stackoverflow" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                handles.push(tokio::spawn(async move {
                    search_stackoverflow(&q, max_results, &http_clone).await
                }));
            }
            "youtube" => {
                let q = input.query.clone();
                handles.push(tokio::spawn(async move {
                    search_youtube(&q, max_results).await
                }));
            }
            _ => {} // skip unknown engines
        }
    }

    // Collect results from all engines
    let mut all_results: Vec<WebSearchResult> = Vec::new();
    let mut engines_used: Vec<String> = Vec::new();
    let mut answer: Option<String> = None;

    for handle in handles {
        match handle.await {
            Ok(Ok((engine_name, mut results, engine_answer))) => {
                if !results.is_empty() {
                    engines_used.push(engine_name);
                }
                all_results.append(&mut results);
                if answer.is_none() && engine_answer.is_some() {
                    answer = engine_answer;
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Search engine error: {}", e);
            }
            Err(e) => {
                tracing::warn!("Search engine task panicked: {}", e);
            }
        }
    }

    // Dedup by URL (keep first occurrence, which has priority ordering)
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut deduped: Vec<WebSearchResult> = Vec::new();
    for result in all_results {
        if seen_urls.insert(result.url.clone()) {
            deduped.push(result);
        }
    }

    // Apply domain filters
    if let Some(ref include) = input.include_domains {
        let include_lower: Vec<String> = include.iter().map(|d| d.to_lowercase()).collect();
        deduped.retain(|r| {
            let domain = r.domain.as_deref().unwrap_or("").to_lowercase();
            include_lower.iter().any(|d| domain.contains(d.as_str()))
        });
    }
    if let Some(ref exclude) = input.exclude_domains {
        let exclude_lower: Vec<String> = exclude.iter().map(|d| d.to_lowercase()).collect();
        deduped.retain(|r| {
            let domain = r.domain.as_deref().unwrap_or("").to_lowercase();
            !exclude_lower.iter().any(|d| domain.contains(d.as_str()))
        });
    }

    // Compute relevance scores for all results
    for result in &mut deduped {
        result.score = Some(compute_relevance_score(result, &input.query));
    }

    // Semantic dedup: remove results with >80% title similarity
    semantic_dedup(&mut deduped);

    // Domain-level dedup: keep highest-scoring result per registrable domain
    domain_dedup(&mut deduped);

    // Sort by score (highest first)
    deduped.sort_by(|a, b| {
        b.score.unwrap_or(0.0)
            .partial_cmp(&a.score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deep mode: scrape each result page for semantic excerpts
    if is_deep && !deduped.is_empty() {
        let obs_settings = settings.browser.obscura.clone();
        if obs_settings.enabled {
            let obscura = crate::obscura::ObscuraManager::new(&obs_settings);
            for result in &mut deduped {
                if let Ok(html) = obscura
                    .fetch_with_all_options(&result.url, "html", false, "load", false, None)
                    .await
                {
                    let excerpt = extract_semantic_excerpt(&html, &result.title, 1500);
                    if !excerpt.is_empty() {
                        result.raw_content = Some(html_to_markdown_rs::convert(&html, None)
                            .ok()
                            .and_then(|r| r.content)
                            .unwrap_or_default());
                        // Upgrade content if the excerpt is better than the snippet
                        if excerpt.len() > result.content.as_deref().unwrap_or("").len() {
                            result.content = Some(excerpt);
                        }
                    }
                }
            }
        }
    }

    // Apply content depth control (skip in deep mode — deep already provides full excerpts)
    if !is_deep {
        for result in &mut deduped {
            result.content = truncate_content(result.content.as_deref(), content_length);
        }
    }

    // Extract highlights if requested (only from substantial text)
    if include_highlights {
        for result in &mut deduped {
            let text = result
                .raw_content
                .as_deref()
                .or(result.content.as_deref())
                .unwrap_or("");
            // Only extract highlights from text with enough content (>200 chars)
            if text.len() > 200 {
                let hl = extract_highlights(text, &input.query, 5);
                if !hl.is_empty() {
                    result.highlights = Some(hl);
                }
            }
        }
    }

    // Chunked content per source: split into paragraphs, score with BM25, return top-N
    if let Some(chunks_per_source) = input.chunks_per_source {
        let cps = chunks_per_source.max(1) as usize;
        for result in &mut deduped {
            let text = result
                .raw_content
                .as_deref()
                .or(result.content.as_deref())
                .unwrap_or("");
            if text.len() < 100 {
                continue;
            }
            let paragraphs = split_paragraphs(text);
            if paragraphs.is_empty() {
                continue;
            }
            let scored = bm25_score_chunks(&paragraphs, &input.query, cps);
            result.chunks = Some(scored.into_iter().map(|c| ScoredChunkOutput {
                content: c.content,
                score: c.score,
                index: c.index,
            }).collect());
        }
    }

    // Truncate to max_results
    deduped.truncate(max_results);

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let count = deduped.len();

    // Answer synthesis: generate from top results if not already provided by engines
    if answer.is_none() && input.include_answer.unwrap_or(false) {
        answer = extractive_answer(&deduped, &input.query);
    }

    // Compute answer confidence: based on engine count and result diversity
    let confidence = if answer.is_some() {
        let engine_count = engines_used.len() as f64;
        let domain_count = deduped.iter()
            .filter_map(|r| r.domain.as_ref())
            .collect::<HashSet<_>>()
            .len() as f64;
        let base = 0.5 + (engine_count * 0.1).min(0.3) + (domain_count * 0.05).min(0.2);
        Some(base.min(1.0))
    } else {
        None
    };

    let output = WebSearchOutput {
        count,
        results: deduped,
        answer,
        confidence,
        meta: WebSearchMeta {
            provider: engines_used.join("+"),
            query: input.query.clone(),
            engines_used,
            response_time_ms: elapsed_ms,
            total_results: count,
            scored: Some(true),
        },
    };

    // Cache the results (skip for deep mode or explicit engines)
    if !is_deep && input.engines.is_none() {
        cache_set(&input.query, topic, max_results, content_length, include_highlights, input.include_answer.unwrap_or(false), &output);
    }

    Ok(output)
}


pub async fn web_scrape(input: WebScrapeInput) -> Result<WebScrapeOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;

    // Determine provider: explicit input, or browser.default from settings
    let provider = input.provider.as_deref().unwrap_or(&settings.browser.default);

    match provider {
        "lightpanda" => web_scrape_lightpanda(&input, &settings).await,
        "obscura" => web_scrape_obscura(&input, &settings).await,
        _ => web_scrape_default(&input, &settings).await,
    }
}

/// Scrape using plain HTTP + html-to-markdown-rs (default provider)
async fn web_scrape_default(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    let cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

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
async fn web_scrape_lightpanda(
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
async fn web_scrape_obscura(
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

fn extract_scrape_output(
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

pub async fn web_crawl(input: WebCrawlInput) -> Result<WebCrawlOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;

    // Use browser.default from settings
    let provider = &settings.browser.default;

    match provider.as_str() {
        "lightpanda" => web_crawl_lightpanda(&input, &settings).await,
        "obscura" => web_crawl_obscura(&input, &settings).await,
        _ => Err(
            "web.crawl requires a headless browser. Set browser.default to 'lightpanda' or 'obscura' in settings.yml"
                .into(),
        ),
    }
}

async fn web_crawl_lightpanda(
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

async fn web_crawl_obscura(
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
fn remove_boilerplate(text: &str) -> String {
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
fn detect_content_type(doc: &scraper::Html) -> Option<String> {
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
fn detect_language(doc: &scraper::Html) -> Option<String> {
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

/// Extract structured content from a URL using Obscura.
/// Supports full extraction (text, metadata, links, images, structured data)
/// or selector-based extraction for specific elements.
/// Batch mode: if `urls` is provided, processes multiple URLs in parallel.
pub async fn web_extract(input: WebExtractInput) -> Result<WebExtractOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let obs_settings = &settings.browser.obscura;

    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use web.extract"
                .into(),
        );
    }

    let clean_content = input.clean_content.unwrap_or(false);
    let start = std::time::Instant::now();
    let url = &input.url;
    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let wait_until = "networkidle";

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

    // Include raw HTML if requested
    let html_output = if input.include_html.unwrap_or(false) {
        Some(html.clone())
    } else {
        None
    };

    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(WebExtractOutput {
        success: true,
        url: url.clone(),
        title,
        content: Some(content),
        markdown,
        html: html_output,
        metadata: Some(metadata),
        structured_data,
        links,
        images,
        elements,
        meta: ExtractMeta {
            url: url.clone(),
            provider: "obscura".into(),
            js_rendered: true,
            elapsed_ms,
        },
    })
}

/// Extract main text content from the document body
fn extract_main_text(doc: &scraper::Html) -> String {
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
fn extract_page_metadata(doc: &scraper::Html) -> ExtractMetadata {
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
fn extract_structured_data(doc: &scraper::Html) -> Option<StructuredData> {
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
fn extract_page_links(doc: &scraper::Html) -> Option<Vec<ExtractedLink>> {
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
fn extract_page_images(doc: &scraper::Html) -> Option<Vec<ExtractedImage>> {
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
fn extract_by_selectors(doc: &scraper::Html, selectors: &[String]) -> Option<Vec<ExtractedElement>> {
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

/// Discover URLs on a website by analyzing sitemap and links.
pub async fn web_map(input: WebMapInput) -> Result<WebMapOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

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
#[cfg(test)]
mod tests {
    use crate::tools::nlp::tokenize;
    use std::time::Duration;
    use super::*;

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello, World! This is a test.", 2, false);
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
        assert!(tokens.contains(&"is".to_string())); // 2-char tokens are kept
        // Single-char tokens filtered out
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn bm25_returns_sorted_by_relevance() {
        let chunks = vec![
            "Rust is a systems programming language".to_string(),
            "Python is great for machine learning".to_string(),
            "Rust async runtime tokio for concurrent programs".to_string(),
        ];
        let scored = bm25_score_chunks(&chunks, "rust async", 2);
        assert_eq!(scored.len(), 2);
        // First result should mention both "rust" and "async"
        assert!(scored[0].content.contains("Rust"));
        assert!(scored[0].score >= scored[1].score);
    }

    #[test]
    fn bm25_empty_input() {
        let scored = bm25_score_chunks(&[], "test", 5);
        assert!(scored.is_empty());
    }

    #[test]
    fn bm25_empty_query() {
        let chunks = vec!["hello".to_string()];
        let scored = bm25_score_chunks(&chunks, "", 5);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].score, 0.0);
    }

    #[test]
    fn jaccard_similarity_identical() {
        assert!((jaccard_similarity("hello world", "hello world") - 1.0).abs() < 0.001);
    }

    #[test]
    fn jaccard_similarity_disjoint() {
        assert!((jaccard_similarity("hello", "world") - 0.0).abs() < 0.001);
    }

    #[test]
    fn jaccard_similarity_partial() {
        let sim = jaccard_similarity("hello world foo", "hello world bar");
        assert!(sim > 0.3 && sim < 0.8);
    }

    #[test]
    fn keyword_relevance_basic() {
        let score = keyword_relevance("Rust async tutorial", "Learn about async in Rust", "rust async");
        assert!(score > 0.5);
    }

    #[test]
    fn domain_authority_known() {
        assert!(domain_authority("github.com") > 0.9);
        assert!(domain_authority("stackoverflow.com") > 0.85);
    }

    #[test]
    fn domain_authority_unknown() {
        assert_eq!(domain_authority("random-blog.example.com"), 0.5);
    }

    #[test]
    fn semantic_dedup_removes_similar() {
        let mut results = vec![
            WebSearchResult {
                title: "How to learn Rust programming".to_string(),
                url: "https://a.com".to_string(),
                content: None, score: Some(0.9), highlights: None,
                raw_content: None, source: None, domain: None,
                published_date: None, favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "How to learn Rust programming language".to_string(),
                url: "https://b.com".to_string(),
                content: None, score: Some(0.7), highlights: None,
                raw_content: None, source: None, domain: None,
                published_date: None, favicon: None,
                chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://a.com");
    }

    #[test]
    fn text_density_basic() {
        let html = "<html><body><p>This is some text content.</p></body></html>";
        let density = text_density(html);
        assert!(density > 0.0 && density <= 1.0);
    }

    #[test]
    fn ddg_redirect_url_parses() {
        let url = "/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc123";
        let result = extract_ddg_redirect_url(url);
        assert_eq!(result.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn ddg_redirect_url_none_for_normal() {
        assert!(extract_ddg_redirect_url("https://normal-url.com").is_none());
    }

    #[test]
    fn extractive_answer_empty_results() {
        let results = vec![];
        assert!(extractive_answer(&results, "test query").is_none());
    }

    #[test]
    fn extractive_answer_empty_query() {
        let results = vec![WebSearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: Some("This is a long sentence about the test topic with enough content to score above the minimum threshold.".to_string()),
            score: Some(0.8),
            highlights: None,
            raw_content: None,
            source: None,
            domain: None,
            published_date: None,
            favicon: None,
                chunks: None,
        }];
        assert!(extractive_answer(&results, "").is_none());
    }

    #[test]
    fn extractive_answer_returns_relevant_sentences() {
        let results = vec![WebSearchResult {
            title: "Rust Programming".to_string(),
            url: "https://example.com".to_string(),
            content: Some("Rust is a systems programming language focused on safety and performance. Rust provides memory safety without garbage collection. The Rust compiler catches many bugs at compile time.".to_string()),
            score: Some(0.9),
            highlights: None,
            raw_content: None,
            source: None,
            domain: None,
            published_date: None,
            favicon: None,
                chunks: None,
        }];
        let answer = extractive_answer(&results, "rust programming language");
        assert!(answer.is_some());
        let text = answer.unwrap();
        assert!(text.to_lowercase().contains("rust"));
    }

    #[test]
    fn extractive_answer_prefers_raw_content() {
        let results = vec![WebSearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: Some("Short snippet.".to_string()),
            score: Some(0.8),
            highlights: None,
            raw_content: Some("This is the full raw content with much more detail about the test topic that should be preferred over the short snippet because it has more sentences to score from.".to_string()),
            source: None,
            domain: None,
            published_date: None,
            favicon: None,
                chunks: None,
        }];
        let answer = extractive_answer(&results, "test topic");
        assert!(answer.is_some());
        // Should use raw_content, not the short snippet
        assert!(answer.unwrap().len() > 50);
    }

    #[test]
    fn jaccard_similarity_three_identical() {
        let sim = jaccard_similarity("hello world test", "hello world test");
        assert!((sim - 1.0).abs() < 0.01);
    }

    #[test]
    fn jaccard_similarity_completely_different() {
        let sim = jaccard_similarity("hello world", "foo bar baz");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn jaccard_similarity_partial_overlap() {
        let sim = jaccard_similarity("hello world test", "hello world foo");
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn bm25_empty_query_zero_score() {
        let chunks = vec!["hello".to_string()];
        let scored = bm25_score_chunks(&chunks, "", 5);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].score, 0.0);
    }

    #[test]
    fn bm25_no_chunks() {
        let chunks: Vec<String> = vec![];
        let scored = bm25_score_chunks(&chunks, "test query", 5);
        assert!(scored.is_empty());
    }

    #[test]
    fn bm25_scores_relevant_higher() {
        let chunks = vec![
            "Rust is a programming language for systems programming".to_string(),
            "The weather is sunny today with clear skies".to_string(),
        ];
        let scored = bm25_score_chunks(&chunks, "rust programming", 2);
        assert_eq!(scored.len(), 2);
        // First chunk should score higher (contains both query terms)
        assert!(scored[0].score >= scored[1].score);
    }

    #[test]
    fn keyword_relevance_title_match() {
        let score = keyword_relevance("Rust Programming Guide", "Some content", "rust programming");
        assert!(score > 0.8); // Title match counts double
    }

    #[test]
    fn keyword_relevance_no_match() {
        let score = keyword_relevance("Weather Forecast", "Sunny skies", "rust programming");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn freshness_score_iso_date() {
        // Today's date should get 1.0
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert_eq!(freshness_score(Some(&today)), 1.0);
    }

    #[test]
    fn freshness_score_unknown() {
        assert_eq!(freshness_score(None), 0.5);
    }

    #[test]
    fn freshness_score_relative_time() {
        assert_eq!(freshness_score(Some("2 hours ago")), 1.0);
        assert_eq!(freshness_score(Some("last week")), 0.6);
        assert_eq!(freshness_score(Some("last month")), 0.4);
    }

    #[test]
    fn truncate_content_none() {
        assert!(truncate_content(None, "standard").is_none());
    }

    #[test]
    fn truncate_content_short() {
        let text = "Short text.";
        let result = truncate_content(Some(text), "standard").unwrap();
        assert_eq!(result, text);
    }

    #[test]
    fn truncate_content_long() {
        let text = "This is a very long text that should be truncated to a shorter version when using standard mode. It needs to be over 150 characters to actually trigger truncation in minimal mode. Adding more filler text to ensure we exceed the threshold. Even more padding here to make sure the test is robust. Extra words for safety.";
        let result = truncate_content(Some(text), "minimal").unwrap();
        assert!(result.len() < text.len());
        assert!(result.ends_with("..."));
    }

    #[test]
    fn route_engines_code_query() {
        let engines = route_engines("general", "rust compiler error");
        assert!(engines.contains(&"github".to_string()));
        assert!(engines.contains(&"stackoverflow".to_string()));
    }

    #[test]
    fn route_engines_news_query() {
        let engines = route_engines("general", "breaking news today");
        assert!(engines.contains(&"duckduckgo".to_string()));
        assert!(engines.contains(&"hackernews".to_string()));
    }

    #[test]
    fn route_engines_explicit_topic() {
        let engines = route_engines("news", "anything");
        assert!(engines.contains(&"duckduckgo".to_string()));
        assert!(engines.contains(&"hackernews".to_string()));
    }

    #[test]
    fn route_engines_video_intent() {
        let engines = route_engines("general", "how to make a video tutorial");
        // Base engines must still be present
        assert!(engines.contains(&"duckduckgo".to_string()));
        assert!(engines.contains(&"wikipedia".to_string()));
        assert!(engines.contains(&"hackernews".to_string()));
        // YouTube added on top for video intent
        assert!(engines.contains(&"youtube".to_string()));
    }

    #[test]
    fn cache_ttl_by_topic() {
        assert_eq!(cache_ttl("news"), Duration::from_secs(300));
        assert_eq!(cache_ttl("code"), Duration::from_secs(43_200));
        assert_eq!(cache_ttl("general"), Duration::from_secs(3_600));
    }

    // ─── extract_highlights tests ─────────────────────────────

    #[test]
    fn extract_highlights_empty_text() {
        let hl = extract_highlights("", "rust programming", 5);
        assert!(hl.is_empty());
    }

    #[test]
    fn extract_highlights_empty_query() {
        let hl = extract_highlights("This is some text.", "", 5);
        assert!(hl.is_empty());
    }

    #[test]
    fn extract_highlights_scoring() {
        let text = "Rust programming language is fast and safe. Python is also good. Rust has memory safety. Java is older.";
        let hl = extract_highlights(text, "rust programming", 5);
        // Sentences containing 'rust' or 'programming' should rank higher
        assert!(!hl.is_empty());
        assert!(hl[0].to_lowercase().contains("rust") || hl[0].to_lowercase().contains("programming"));
    }

    #[test]
    fn extract_highlights_max_limit() {
        let text = "First sentence about rust. Second sentence about rust. Third sentence about rust. Fourth sentence about rust. Fifth sentence about rust. Sixth sentence about rust.";
        let hl = extract_highlights(text, "rust", 2);
        assert!(hl.len() <= 2);
    }

    #[test]
    fn extract_highlights_filters_short_sentences() {
        // Sentences shorter than 30 chars are excluded
        let text = "Short. This is a longer sentence about rust programming that should be included.";
        let hl = extract_highlights(text, "rust", 5);
        assert!(!hl.is_empty());
        assert!(hl.iter().all(|s| s.len() > 30));
    }

    // ─── domain_dedup tests ───────────────────────────────────

    #[test]
    fn domain_dedup_removes_duplicates() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "Article 1".into(),
                url: "https://www.bbc.com/news/1".into(),
                content: Some("content1".into()),
                score: Some(0.5),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.com".into()),
                published_date: None, favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "Article 2".into(),
                url: "https://www.bbc.com/news/2".into(),
                content: Some("content2".into()),
                score: Some(0.8),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.com".into()),
                published_date: None, favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Article 2"); // higher score wins
    }

    #[test]
    fn domain_dedup_preserves_different_domains() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "From BBC".into(),
                url: "https://bbc.com/1".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.com".into()),
                published_date: None, favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "From Reuters".into(),
                url: "https://reuters.com/1".into(),
                content: Some("c".into()),
                score: Some(0.6),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("reuters.com".into()),
                published_date: None, favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 2); // different domains kept
    }

    #[test]
    fn domain_dedup_multipart_tld() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "BBC UK 1".into(),
                url: "https://www.bbc.co.uk/news/1".into(),
                content: Some("c".into()),
                score: Some(0.4),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("www.bbc.co.uk".into()),
                published_date: None, favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "BBC UK 2".into(),
                url: "https://bbc.co.uk/sport/1".into(),
                content: Some("c".into()),
                score: Some(0.9),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.co.uk".into()),
                published_date: None, favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "BBC UK 2"); // higher score
    }

    #[test]
    fn domain_dedup_no_domain() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "No domain".into(),
                url: "https://example.com/1".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: None,
                published_date: None, favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 1); // no domain = no dedup
    }

    // ─── semantic_dedup tests ──────────────────────────────────

    #[test]
    fn semantic_dedup_identical_titles() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "Rust programming language tutorial".into(),
                url: "https://a.com".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("a.com".into()),
                published_date: None, favicon: None, chunks: None,
            },
            WebSearchResult {
                title: "Rust programming language tutorial".into(),
                url: "https://b.com".into(),
                content: Some("c".into()),
                score: Some(0.8),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("b.com".into()),
                published_date: None, favicon: None, chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://b.com"); // higher score wins
    }

    #[test]
    fn semantic_dedup_different_titles() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "Rust programming".into(),
                url: "https://a.com".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("a.com".into()),
                published_date: None, favicon: None, chunks: None,
            },
            WebSearchResult {
                title: "Python tutorial".into(),
                url: "https://b.com".into(),
                content: Some("c".into()),
                score: Some(0.6),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("b.com".into()),
                published_date: None, favicon: None, chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 2); // different titles kept
    }

    #[test]
    fn semantic_dedup_below_threshold() {
        use super::WebSearchResult;
        // Similar but below 0.8 Jaccard threshold
        let mut results = vec![
            WebSearchResult {
                title: "Rust is a systems programming language".into(),
                url: "https://a.com".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("a.com".into()),
                published_date: None, favicon: None, chunks: None,
            },
            WebSearchResult {
                title: "Rust is a fast compiled language".into(),
                url: "https://b.com".into(),
                content: Some("c".into()),
                score: Some(0.6),
                highlights: None, raw_content: None,
                source: Some("ddg".into()),
                domain: Some("b.com".into()),
                published_date: None, favicon: None, chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 2); // below 0.8 threshold, both kept
    }

    // ─── compute_relevance_score tests ──────────────────────────

    #[test]
    fn compute_relevance_score_title_match_high() {
        use super::WebSearchResult;
        let result = WebSearchResult {
            title: "Rust programming language".into(),
            url: "https://rust-lang.org".into(),
            content: Some("Rust is a systems programming language.".into()),
            score: None,
            highlights: None, raw_content: None,
            source: Some("wikipedia".into()),
            domain: Some("wikipedia.org".into()),
            published_date: None, favicon: None, chunks: None,
        };
        let score = compute_relevance_score(&result, "rust programming");
        assert!(score > 0.6, "Expected high score for title match, got {}", score);
    }

    #[test]
    fn compute_relevance_score_no_match_low() {
        use super::WebSearchResult;
        let result = WebSearchResult {
            title: "Cooking recipes".into(),
            url: "https://cooking.com".into(),
            content: Some("Delicious recipes for dinner.".into()),
            score: None,
            highlights: None, raw_content: None,
            source: Some("ddg".into()),
            domain: Some("cooking.com".into()),
            published_date: None, favicon: None, chunks: None,
        };
        let score = compute_relevance_score(&result, "rust programming");
        assert!(score < 0.5, "Expected low score for no match, got {}", score);
    }

    #[test]
    fn compute_relevance_score_freshness_boost() {
        use super::WebSearchResult;
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let result = WebSearchResult {
            title: "Rust update".into(),
            url: "https://example.com".into(),
            content: Some("Rust gets new features.".into()),
            score: None,
            highlights: None, raw_content: None,
            source: Some("ddg".into()),
            domain: Some("example.com".into()),
            published_date: Some(today), favicon: None, chunks: None,
        };
        let score = compute_relevance_score(&result, "rust");
        // Fresh content should get a boost
        assert!(score > 0.5, "Expected freshness boost, got {}", score);
    }

    #[test]
    fn compute_relevance_score_authority_boost() {
        use super::WebSearchResult;
        let result = WebSearchResult {
            title: "Rust docs".into(),
            url: "https://doc.rust-lang.org".into(),
            content: Some("Official Rust documentation.".into()),
            score: None,
            highlights: None, raw_content: None,
            source: Some("ddg".into()),
            domain: Some("docs.rs".into()),
            published_date: None, favicon: None, chunks: None,
        };
        let score = compute_relevance_score(&result, "rust docs");
        // docs.rs has high authority (0.85)
        assert!(score > 0.6, "Expected authority boost, got {}", score);
    }

    // ─── chunked content tests ─────────────────────────────────

    #[test]
    fn bm25_chunk_scoring_returns_top_k() {
        let chunks = vec![
            "Rust is a systems programming language focused on safety and performance.".into(),
            "Python is a popular general-purpose language used in data science.".into(),
            "Rust ownership prevents data races at compile time with zero cost.".into(),
        ];
        let scored = bm25_score_chunks(&chunks, "rust safety", 2);
        assert_eq!(scored.len(), 2);
        // The Python chunk (no query terms) should not be in top 2
        assert!(scored.iter().all(|c| !c.content.contains("Python")),
            "Python chunk should rank lower than Rust chunks");
        assert!(scored[0].score > 0.0);
    }

    #[test]
    fn bm25_chunk_scoring_preserves_index() {
        let chunks = vec!["hello world".into(), "foo bar baz".into()];
        let scored = bm25_score_chunks(&chunks, "hello", 2);
        assert_eq!(scored.len(), 2);
        let hello_chunk = scored.iter().find(|c| c.content.contains("hello")).unwrap();
        let foo_chunk = scored.iter().find(|c| c.content.contains("foo")).unwrap();
        assert!(hello_chunk.score > foo_chunk.score);
        assert_eq!(hello_chunk.index, 0);
        assert_eq!(foo_chunk.index, 1);
    }

    #[test]
    fn bm25_single_chunk() {
        let chunks = vec!["Only one paragraph here.".into()];
        let scored = bm25_score_chunks(&chunks, "anything", 5);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].index, 0);
    }

    #[test]
    fn split_paragraphs_normal() {
        let text = "First paragraph with enough content to pass the filter and be included in results."
            .to_string()
            + "\n\n"
            + "Second paragraph also with enough content to survive the 40 char minimum filter."
            + "\n\n"
            + "Third paragraph of sufficient length that it should not be filtered out by the length check.";
        let paragraphs = super::split_paragraphs(&text);
        assert_eq!(paragraphs.len(), 3, "Expected 3 paragraphs after splitting");
        assert!(paragraphs[0].starts_with("First paragraph"));
        assert!(paragraphs[1].starts_with("Second paragraph"));
        assert!(paragraphs[2].starts_with("Third paragraph"));
    }

    #[test]
    fn split_paragraphs_filters_short() {
        let text = "A short line.\n\nAnother short one.";
        let paragraphs = super::split_paragraphs(text);
        assert!(paragraphs.is_empty(), "Short fragments should be filtered out");
    }

    #[test]
    fn split_paragraphs_collapses_single_newlines() {
        let text = "Line one of paragraph.\nLine two of paragraph.\nLine three of paragraph.";
        let paragraphs = super::split_paragraphs(text);
        // No double newlines, so the whole text is one "paragraph"
        // but single newlines are collapsed into spaces
        assert_eq!(paragraphs.len(), 1);
        assert!(paragraphs[0].contains("Line one"));
        assert!(!paragraphs[0].contains('\n'), "Single newlines should be collapsed");
    }
}
