use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::*;
use crate::tools::types::*;
use std::collections::HashMap;

/// Extract internal links from a parsed HTML document
fn extract_internal_links(
    doc: &scraper::Html,
    sel: &scraper::Selector,
    base_url: &url::Url,
    base_host: &str,
) -> Vec<String> {
    doc.select(sel)
        .filter_map(|el| el.attr("href"))
        .filter_map(|href| {
            url::Url::parse(href)
                .ok()
                .or_else(|| base_url.join(href).ok())
        })
        .map(|u| u.to_string())
        .filter(|url_str| {
            url::Url::parse(url_str)
                .ok()
                .and_then(|u| u.host_str().map(|s| s.to_string()))
                .unwrap_or_default()
                == base_host
        })
        .collect()
}

/// Search the web in realtime. Uses Tavily or Firecrawl API.
pub async fn web_search(input: WebSearchInput) -> Result<WebSearchOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let provider = input.provider.as_deref().unwrap_or("auto");

    // Try Tavily first
    if provider == "auto" || provider == "tavily" {
        if let Some(ref tavily) = settings.tavily {
            if tavily.enabled {
                if let Some(ref api_key) = tavily.api_key {
                    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
                    let http = HttpClient::new(&settings.http, &cache_dir);
                    
                    let mut body = serde_json::json!({
                        "api_key": api_key,
                        "query": input.query,
                        "max_results": input.max_results.unwrap_or(10),
                        "topic": input.topic.as_deref().unwrap_or("general"),
                    });
                    
                    if let Some(ref domains) = input.include_domains {
                        if !domains.is_empty() {
                            body["include_domains"] = serde_json::json!(domains);
                        }
                    }
                    if let Some(ref domains) = input.exclude_domains {
                        if !domains.is_empty() {
                            body["exclude_domains"] = serde_json::json!(domains);
                        }
                    }
                    if let Some(days) = input.days {
                        body["days"] = serde_json::json!(days);
                    }
                    if let Some(answer) = input.include_answer {
                        body["include_answer"] = serde_json::json!(answer);
                    }
                    
                    match http.post_json("https://api.tavily.com/search", &body, None).await {
                        Ok(outcome) => {
                            if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                                    let results: Vec<WebSearchResult> = json["results"]
                                        .as_array()
                                        .map(|arr| arr.iter().map(|r| WebSearchResult {
                                            title: r["title"].as_str().unwrap_or("").to_string(),
                                            url: r["url"].as_str().unwrap_or("").to_string(),
                                            content: r["content"].as_str().map(|s| s.to_string()),
                                            score: r["score"].as_f64(),
                                            raw_content: r["raw_content"].as_str().map(|s| s.to_string()),
                                        }).collect())
                                        .unwrap_or_default();
                                    let answer = json["answer"].as_str().map(|s| s.to_string());
                                    let count = results.len();
                                    return Ok(WebSearchOutput {
                                        count,
                                        results,
                                        answer,
                                        meta: WebSearchMeta {
                                            provider: "tavily".into(),
                                            query: input.query,
                                        },
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            if provider == "tavily" {
                                return Err(format!("Tavily search failed: {}", e));
                            }
                            tracing::warn!("Tavily search failed, trying Firecrawl: {}", e);
                        }
                    }
                }
            }
        }
    }

    // Fallback to Firecrawl
    if let Some(ref firecrawl) = settings.firecrawl {
        if firecrawl.enabled {
            if let Some(ref api_key) = firecrawl.api_key {
                let query_enc = urlencoding(&input.query);
                let url = format!(
                    "https://api.firecrawl.dev/v1/search?query={}&limit={}",
                    query_enc, input.max_results.unwrap_or(10)
                );
                let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
                let http = HttpClient::new(&settings.http, &cache_dir);
                match http.fetch(&url, Some(&HashMap::from([("Authorization".into(), format!("Bearer {}", api_key))])), "bypass").await {
                    Ok(outcome) => {
                        if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                                let results: Vec<WebSearchResult> = json["data"]["web"]
                                    .as_array()
                                    .map(|arr| arr.iter().map(|r| WebSearchResult {
                                        title: r["title"].as_str().unwrap_or("").to_string(),
                                        url: r["url"].as_str().unwrap_or("").to_string(),
                                        content: r["content"].as_str().map(|s| s.to_string()),
                                        score: r["score"].as_f64(),
                                        raw_content: None,
                                    }).collect())
                                    .unwrap_or_default();
                                let count = results.len();
                                return Ok(WebSearchOutput {
                                    count,
                                    results,
                                    answer: None,
                                    meta: WebSearchMeta {
                                        provider: "firecrawl".into(),
                                        query: input.query,
                                    },
                                });
                            }
                        }
                    }
                    Err(e) => { tracing::warn!("Firecrawl search failed: {}", e); }
                }
            }
        }
    }

    Err("No web search provider available. Configure Tavily or Firecrawl in settings.yml.".into())
}

pub async fn web_scrape(input: WebScrapeInput) -> Result<WebScrapeOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let provider = input.provider.as_deref().unwrap_or("default");

    match provider {
        "lightpanda" => web_scrape_lightpanda(&input, &settings).await,
        "tavily" => web_scrape_tavily(&input, &settings).await,
        "firecrawl" => web_scrape_firecrawl(&input, &settings).await,
        _ => web_scrape_default(&input, &settings).await,
    }
}

/// Scrape using plain HTTP + html-to-markdown-rs (default provider)
async fn web_scrape_default(input: &WebScrapeInput, settings: &crate::types::Settings) -> Result<WebScrapeOutput, String> {
    let cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let body = match http.fetch(&input.url, None, "bypass").await {
        Ok(outcome) => match outcome {
            http_mod::FetchOutcome::Response(resp, _, _) => {
                if resp.status < 200 || resp.status >= 400 {
                    return Err(format!("HTTP {} for URL: {}", resp.status, input.url));
                }
                resp.body_text
            }
            http_mod::FetchOutcome::Cached(_) => {
                return Err(format!("Server returned 304 Not Modified for URL: {}. Try again later.", input.url));
            }
        },
        Err(e) => return Err(format!("Scrape failed: {}", e)),
    };

    extract_scrape_output(&input.url, &body, "default", input.formats.as_deref())
}

/// Scrape using Lightpanda headless browser (JS rendering)
async fn web_scrape_lightpanda(input: &WebScrapeInput, settings: &crate::types::Settings) -> Result<WebScrapeOutput, String> {
    if !settings.lightpanda.enabled {
        return Err("Lightpanda is not enabled. Set lightpanda.enabled=true in settings.yml to use provider='lightpanda'".into());
    }

    let lp = crate::lightpanda::LightpandaManager::new(&settings.lightpanda);
    let obey_robots = settings.lightpanda.obey_robots;
    let dump_format = "markdown";
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let include_frames = input.include_frames.unwrap_or(false);

    let body = lp.fetch_with_all_options(
        &input.url,
        dump_format,
        obey_robots,
        wait_until,
        include_frames,
        input.wait_selector.as_deref(),
        input.strip_mode.as_deref(),
        input.structured_data.unwrap_or(false),
    ).await.map_err(|e| format!("Lightpanda scrape failed: {}", e))?;

    extract_scrape_output(&input.url, &body, "lightpanda", input.formats.as_deref())
}

async fn web_scrape_tavily(input: &WebScrapeInput, settings: &crate::types::Settings) -> Result<WebScrapeOutput, String> {
    let tavily = settings.tavily.as_ref().ok_or("Tavily not configured")?;
    if !tavily.enabled {
        return Err("Tavily is not enabled. Set tavily.enabled=true in settings.yml".into());
    }
    let api_key = tavily.api_key.as_ref().ok_or("Tavily API key not configured")?;

    let cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let body = serde_json::json!({
        "api_key": api_key,
        "urls": [input.url],
        "include_images": false,
    });

    let headers = HashMap::from([("Content-Type".into(), "application/json".into())]);

    match http.post_json("https://api.tavily.com/extract", &body, Some(&headers)).await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            if resp.status >= 400 {
                return Err(format!("Tavily extract failed (HTTP {}): {}", resp.status, resp.body_text));
            }
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("Tavily parse error: {}", e))?;

            let results = json["results"].as_array().cloned().unwrap_or_default();
            if let Some(first) = results.first() {
                let raw_content = first["raw_content"].as_str().unwrap_or("");
                let markdown = if raw_content.is_empty() {
                    first["content"].as_str().unwrap_or("").to_string()
                } else {
                    raw_content.to_string()
                };

                return Ok(WebScrapeOutput {
                    success: true,
                    url: input.url.clone(),
                    title: first["title"].as_str().map(|s| s.to_string()),
                    markdown: Some(markdown),
                    metadata: None,
                    meta: ScrapeMeta {
                        url: input.url.clone(),
                        status: 200,
                        content_type: None,
                        elapsed_ms: 0,
                        js_rendered: false,
                    },
                });
            }

            Err("Tavily extract returned no results".into())
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => Err("Unexpected cache hit for Tavily extract".into()),
        Err(e) => Err(format!("Tavily extract failed: {}", e)),
    }
}

async fn web_scrape_firecrawl(input: &WebScrapeInput, settings: &crate::types::Settings) -> Result<WebScrapeOutput, String> {
    let firecrawl = settings.firecrawl.as_ref().ok_or("Firecrawl not configured")?;
    if !firecrawl.enabled {
        return Err("Firecrawl is not enabled. Set firecrawl.enabled=true in settings.yml".into());
    }
    let api_key = firecrawl.api_key.as_ref().ok_or("Firecrawl API key not configured")?;

    let cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let default_formats = vec!["markdown".to_string()];
    let formats = input.formats.as_ref().unwrap_or(&default_formats);
    let body = serde_json::json!({
        "url": input.url,
        "formats": formats,
        "onlyMainContent": true,
    });

    let headers = HashMap::from([
        ("Authorization".into(), format!("Bearer {}", api_key)),
        ("Content-Type".into(), "application/json".into()),
    ]);

    match http.post_json("https://api.firecrawl.dev/v1/scrape", &body, Some(&headers)).await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            if resp.status >= 400 {
                return Err(format!("Firecrawl scrape failed (HTTP {}): {}", resp.status, resp.body_text));
            }
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("Firecrawl parse error: {}", e))?;

            let data = &json["data"];
            let markdown = data["markdown"].as_str().map(|s| s.to_string());
            let title = data["metadata"]["title"].as_str().map(|s| s.to_string());
            let description = data["metadata"]["description"].as_str().map(|s| s.to_string());
            let og_title = data["metadata"]["ogTitle"].as_str().map(|s| s.to_string());
            let og_description = data["metadata"]["ogDescription"].as_str().map(|s| s.to_string());

            Ok(WebScrapeOutput {
                success: true,
                url: input.url.clone(),
                title,
                markdown,
                metadata: Some(ScrapeStructuredData {
                    description,
                    og_title,
                    og_description,
                    links_count: 0,
                    headings: Vec::new(),
                }),
                meta: ScrapeMeta {
                    url: input.url.clone(),
                    status: 200,
                    content_type: None,
                    elapsed_ms: 0,
                    js_rendered: false,
                },
            })
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => Err("Unexpected cache hit for Firecrawl scrape".into()),
        Err(e) => Err(format!("Firecrawl scrape failed: {}", e)),
    }
}

fn extract_scrape_output(url: &str, body: &str, _provider: &str, _formats: Option<&[String]>) -> Result<WebScrapeOutput, String> {
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
            main_content.split_whitespace().take(2000).collect::<Vec<_>>().join(" ")
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
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let provider = input.provider.as_deref().unwrap_or("lightpanda");

    match provider {
        "firecrawl" => web_crawl_firecrawl(&input, &settings).await,
        _ => web_crawl_lightpanda(&input, &settings).await,
    }
}

async fn web_crawl_lightpanda(input: &WebCrawlInput, settings: &crate::types::Settings) -> Result<WebCrawlOutput, String> {
    let lp_settings = settings.lightpanda.clone();
    if !lp_settings.enabled {
        return Err("Lightpanda is not enabled. Set lightpanda.enabled=true in settings.yml to use web.crawl".into());
    }

    let lp = crate::lightpanda::LightpandaManager::new(&lp_settings);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);
    let obey_robots = input.obey_robots.unwrap_or(lp_settings.obey_robots);
    let dump_format = input.dump_format.as_deref().unwrap_or("markdown");
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let include_frames = input.include_frames.unwrap_or(false);
    let wait_selector = input.wait_selector.as_deref();
    let strip_mode = input.strip_mode.as_deref();

    let _binary = lp.ensure_ready().await
        .map_err(|e| format!("Lightpanda not ready: {}", e))?;

    let content = lp.fetch_with_all_options(
        &input.url, dump_format, obey_robots, wait_until, include_frames,
        wait_selector, strip_mode, false,
    ).await
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

        let mut queue: std::collections::VecDeque<(String, i32)> = std::collections::VecDeque::new();
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

            match lp.fetch_with_all_options(&url_str, dump_format, obey_robots, wait_until, include_frames, wait_selector, strip_mode, false).await {
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
                            if !visited.contains(&link_url) && pages.len() + queue.len() < max_pages as usize {
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

async fn web_crawl_firecrawl(input: &WebCrawlInput, settings: &crate::types::Settings) -> Result<WebCrawlOutput, String> {
    let firecrawl = settings.firecrawl.as_ref().ok_or("Firecrawl not configured")?;
    if !firecrawl.enabled {
        return Err("Firecrawl is not enabled. Set firecrawl.enabled=true in settings.yml".into());
    }
    let api_key = firecrawl.api_key.as_ref().ok_or("Firecrawl API key not configured")?;

    let cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);

    let body = serde_json::json!({
        "url": input.url,
        "limit": max_pages,
        "maxDepth": max_depth,
        "scrapeOptions": {
            "formats": ["markdown"],
            "onlyMainContent": true,
        },
    });

    let headers = HashMap::from([
        ("Authorization".into(), format!("Bearer {}", api_key)),
        ("Content-Type".into(), "application/json".into()),
    ]);

    let job_id = match http.post_json("https://api.firecrawl.dev/v1/crawl", &body, Some(&headers)).await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            if resp.status >= 400 {
                return Err(format!("Firecrawl crawl start failed (HTTP {}): {}", resp.status, resp.body_text));
            }
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("Firecrawl parse error: {}", e))?;
            json["id"].as_str()
                .ok_or_else(|| format!("Firecrawl crawl returned no job ID: {}", resp.body_text))?
                .to_string()
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => return Err("Unexpected cache hit for Firecrawl crawl".into()),
        Err(e) => return Err(format!("Firecrawl crawl start failed: {}", e)),
    };

    let poll_url = format!("https://api.firecrawl.dev/v1/crawl/{}", job_id);
    let mut attempts = 0u32;
    let max_attempts = 120;

    loop {
        if attempts >= max_attempts {
            return Err(format!("Firecrawl crawl timed out after {} polls", max_attempts));
        }
        attempts += 1;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        match http.fetch(&poll_url, Some(&headers), "bypass").await {
            Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
                if resp.status >= 400 {
                    return Err(format!("Firecrawl poll failed (HTTP {}): {}", resp.status, resp.body_text));
                }
                let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                    .map_err(|e| format!("Firecrawl poll parse error: {}", e))?;

                let status = json["status"].as_str().unwrap_or("");
                match status {
                    "completed" => {
                        let data = json["data"].as_array().cloned().unwrap_or_default();
                        let pages: Vec<CrawledPage> = data.iter().enumerate().map(|(i, item)| {
                            CrawledPage {
                                url: item["metadata"]["sourceURL"].as_str()
                                    .or(item["url"].as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                title: item["metadata"]["title"].as_str().map(|s| s.to_string()),
                                content: item["markdown"].as_str().unwrap_or("").to_string(),
                                depth: i as i32,
                                status: "ok".to_string(),
                            }
                        }).collect();

                        let count = pages.len();
                        return Ok(WebCrawlOutput {
                            success: true,
                            start_url: input.url.clone(),
                            pages,
                            count,
                            meta: WebCrawlMeta {
                                provider: "firecrawl".to_string(),
                                max_depth,
                                max_pages,
                                obey_robots: true,
                                dump_format: "markdown".to_string(),
                                wait_until: "networkidle".to_string(),
                                include_frames: false,
                            },
                        });
                    }
                    "failed" => {
                        let error = json["error"].as_str().unwrap_or("unknown error");
                        return Err(format!("Firecrawl crawl failed: {}", error));
                    }
                    _ => continue,
                }
            }
            Ok(http_mod::FetchOutcome::Cached(_)) => return Err("Unexpected cache hit for Firecrawl poll".into()),
            Err(e) => return Err(format!("Firecrawl poll failed: {}", e)),
        }
    }
}

/// Discover URLs on a website by analyzing sitemap and links.
pub async fn web_map(input: WebMapInput) -> Result<WebMapOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let base_url = input.url.trim_end_matches('/');
    let sitemap_url = format!("{}/sitemap.xml", base_url);

    let mut links: Vec<WebMapLink> = Vec::new();
    // Try sitemap.xml
    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&sitemap_url, None, "bypass").await {
            let doc = scraper::Html::parse_document(&resp.body_text);
            // Try to extract <loc> elements from sitemap XML
            for line in resp.body_text.lines() {
                if line.contains("<loc>") {
                    if let Some(start) = line.find("<loc>") {
                        let rest = &line[start + 5..];
                        if let Some(end) = rest.find("</loc>") {
                            let url = &rest[..end];
                            links.push(WebMapLink { url: url.to_string(), title: None });
                        }
                    }
                }
            }
            // Also get <url> elements
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
                                links.push(WebMapLink { url: url_str, title });
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
                || link.title.as_ref().is_some_and(|t| t.to_lowercase().contains(&search_lower))
        });
    }

    let limit = input.limit.unwrap_or(100) as usize;
    links.truncate(limit);
    let count = links.len();

    Ok(WebMapOutput {
        success: true,
        url: input.url,
        count,
        links,
        meta: WebMapMeta {
            provider: "sitemap-parser".to_string(),
            limit,
        },
    })
}
