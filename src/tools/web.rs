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
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let provider = input.provider.as_deref().unwrap_or("auto");

    // Try Tavily first
    if provider == "auto" || provider == "tavily" {
        if let Some(ref tavily) = settings.tavily {
            if tavily.enabled {
                if let Some(ref api_key) = tavily.api_key {
                    let cache_dir =
                        http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
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

                    match http
                        .post_json("https://api.tavily.com/search", &body, None)
                        .await
                    {
                        Ok(outcome) => {
                            if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                                match serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                                    Ok(json) => {
                                        let results: Vec<WebSearchResult> = json["results"]
                                            .as_array()
                                            .map(|arr| {
                                                arr.iter()
                                                    .map(|r| WebSearchResult {
                                                        title: r["title"]
                                                            .as_str()
                                                            .unwrap_or("")
                                                            .to_string(),
                                                        url: r["url"]
                                                            .as_str()
                                                            .unwrap_or("")
                                                            .to_string(),
                                                        content: r["content"]
                                                            .as_str()
                                                            .map(|s| s.to_string()),
                                                        score: r["score"].as_f64(),
                                                        raw_content: r["raw_content"]
                                                            .as_str()
                                                            .map(|s| s.to_string()),
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        let answer =
                                            json["answer"].as_str().map(|s| s.to_string());
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
                                    Err(parse_err) => {
                                        // Tavily returned a non-JSON body (e.g.,
                                        // HTML error page, empty response, or a
                                        // misconfigured API key redirect).
                                        // Log at WARN so users can diagnose
                                        // misconfigured keys instead of silently
                                        // falling through to Firecrawl.
                                        tracing::warn!(
                                            "Tavily response parse failed (status={}, body_len={}): {}. Falling through to Firecrawl.",
                                            resp.status,
                                            resp.body_text.len(),
                                            parse_err
                                        );
                                    }
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
                    query_enc,
                    input.max_results.unwrap_or(10)
                );
                let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
                let http = HttpClient::new(&settings.http, &cache_dir);
                match http
                    .fetch(
                        &url,
                        Some(&HashMap::from([(
                            "Authorization".into(),
                            format!("Bearer {}", api_key),
                        )])),
                        "bypass",
                    )
                    .await
                {
                    Ok(outcome) => {
                        if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                            if let Ok(json) =
                                serde_json::from_str::<serde_json::Value>(&resp.body_text)
                            {
                                let results: Vec<WebSearchResult> = json["data"]["web"]
                                    .as_array()
                                    .map(|arr| {
                                        arr.iter()
                                            .map(|r| WebSearchResult {
                                                title: r["title"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string(),
                                                url: r["url"].as_str().unwrap_or("").to_string(),
                                                content: r["content"]
                                                    .as_str()
                                                    .map(|s| s.to_string()),
                                                score: r["score"].as_f64(),
                                                raw_content: None,
                                            })
                                            .collect()
                                    })
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
                    Err(e) => {
                        tracing::warn!("Firecrawl search failed: {}", e);
                    }
                }
            }
        }
    }

    Err("No web search provider available. Configure Tavily or Firecrawl in settings.yml.".into())
}

pub async fn web_scrape(input: WebScrapeInput) -> Result<WebScrapeOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let provider = input.provider.as_deref().unwrap_or("default");

    match provider {
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
            let http_mod::FetchOutcome::Response(resp, _, _) = outcome
                else { unreachable!("bypass cache mode never returns Cached") };
            if resp.status < 200 || resp.status >= 400 {
                return Err(format!("HTTP {} for URL: {}", resp.status, input.url));
            }
            resp.body_text
        }
        Err(e) => return Err(format!("Scrape failed: {}", e)),
    };

    extract_scrape_output(&input.url, &body, "default", input.formats.as_deref())
}

/// Scrape using Obscura headless browser (JS rendering)
async fn web_scrape_obscura(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    if !settings.obscura.enabled {
        return Err("Obscura is not enabled. Set obscura.enabled=true in settings.yml to use provider='obscura'".into());
    }

    let obscura = crate::obscura::ObscuraManager::new(&settings.obscura);
    let obey_robots = settings.obscura.obey_robots;
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
    web_crawl_obscura(&input, &settings).await
}

async fn web_crawl_obscura(
    input: &WebCrawlInput,
    settings: &crate::types::Settings,
) -> Result<WebCrawlOutput, String> {
    let obscura_settings = settings.obscura.clone();
    if !obscura_settings.enabled {
        return Err(
            "Obscura is not enabled. Set obscura.enabled=true in settings.yml to use web.crawl"
                .into(),
        );
    }

    let obscura = crate::obscura::ObscuraManager::new(&obscura_settings);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);
    let obey_robots = input.obey_robots.unwrap_or(obscura_settings.obey_robots);
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
