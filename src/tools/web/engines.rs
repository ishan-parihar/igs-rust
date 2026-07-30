//! Search engine implementations (DDG, Wikipedia, GitHub, HN, SO, YouTube).
//! All engines are free, no API keys required.

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types::*;
use super::scoring::extract_domain;
use super::readability::extract_ddg_redirect_url;
use std::collections::HashMap;

// ─── YouTube Search Engine (via yt-dlp) ──────────────────────

/// Search YouTube via yt-dlp (already available on the system).
/// Returns (engine_name, results, answer). Key-free.
pub(super) async fn search_youtube(
    query: &str,
    max_results: usize,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let limit = max_results.min(10);
    let search_term = format!("ytsearch{}:{}", limit, query);

    // Fail gracefully — don't break web.search if yt-dlp is missing
    let output = match tokio::process::Command::new("yt-dlp")
        .args([
            &search_term,
            "--flat-playlist",
            "--print",
            "%(id)s|||%(title)s|||%(channel)s|||%(duration_string)s",
        ])
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return Ok(("youtube".to_string(), vec![], None)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("ERROR") {
            tracing::warn!("yt-dlp error: {}", stderr.trim());
            return Ok(("youtube".to_string(), vec![], None));
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.splitn(4, "|||").collect();
        if parts.len() < 3 { continue; }

        let video_id = parts[0];
        let title = parts[1];
        let channel = parts[2];
        let duration = if parts.len() > 3 && !parts[3].is_empty() {
            Some(parts[3].to_string())
        } else {
            None
        };

        let url = format!("https://www.youtube.com/watch?v={}", video_id);
        let content = format!("📺 {} | {}", channel, duration.unwrap_or_default());

        results.push(WebSearchResult {
            title: title.to_string(),
            url,
            content: Some(content),
            score: Some(0.7),
            highlights: None,
            raw_content: None,
            source: Some("youtube".to_string()),
            domain: Some("youtube.com".to_string()),
            published_date: None,
            favicon: None,
                chunks: None,
        });
    }

    Ok(("youtube".to_string(), results, None))
}

/// Search for images via Wikimedia Commons REST API (key-free).
/// Uses the Wikimedia Commons API to find freely licensed images.
/// No API key required — Wikimedia is completely open.
pub async fn web_image_search(input: WebImageSearchInput) -> Result<WebImageSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;

    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    let max_results = input.max_results.unwrap_or(10).min(30) as usize;
    let query_encoded = url::form_urlencoded::byte_serialize(input.query.as_bytes()).collect::<String>();

    let start = std::time::Instant::now();

    // Wikimedia Commons API — search for images in namespace 6 (File)
    let search_url = format!(
        "https://commons.wikimedia.org/w/api.php?action=query&generator=search&gsrsearch=file:{}&gsrnamespace=6&prop=imageinfo&iiprop=url|extmetadata|size&iiurlwidth=800&format=json",
        query_encoded
    );

    let mut results = Vec::new();
    let mut total_available = 0;

    match http.fetch(&search_url, None, "bypass").await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                // Get total count if available, fallback to pages count
                if let Some(query_info) = json["query"]["searchinfo"].as_object() {
                    total_available = query_info["totalhits"].as_u64().unwrap_or(0) as usize;
                }

                if let Some(pages) = json["query"]["pages"].as_object() {
                    // Only count image MIME types (exclude PDFs, SVGs with no raster, etc.)

                    for (_, page) in pages {
                        if results.len() >= max_results {
                            break;
                        }

                        let title = page["title"].as_str().unwrap_or("Untitled").to_string();

                        // Skip non-image files (PDFs, documents, etc.)
                        let title_lower = title.to_lowercase();
                        if title_lower.ends_with(".pdf")
                            || title_lower.ends_with(".svg")
                            || title_lower.ends_with(".mid")
                            || title_lower.ends_with(".midi")
                        {
                            continue;
                        }

                        // Extract image URL from imageinfo
                        if let Some(imageinfo) = page["imageinfo"].as_array() {
                            if let Some(info) = imageinfo.first() {
                                let url = info["url"].as_str().unwrap_or("").to_string();
                                let thumb_url = info["thumburl"].as_str().map(|s| s.to_string());
                                let width = info["width"].as_u64().map(|w| w as u32);
                                let height = info["height"].as_u64().map(|h| h as u32);

                                // Source page URL on Wikimedia Commons
                                let source_url = Some(format!(
                                    "https://commons.wikimedia.org/wiki/{}",
                                    title.replace(' ', "_")
                                ));

                                if !url.is_empty() {
                                    // Extract image description from Wikimedia extmetadata
                                let description = info["extmetadata"]
                                    .as_object()
                                    .and_then(|em| em.get("ImageDescription"))
                                    .and_then(|desc| desc.get("value"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| {
                                        // Strip HTML tags using scraper for clean text extraction
                                        let doc = scraper::Html::parse_fragment(s);
                                        doc.root_element().text().collect::<Vec<_>>().join(" ")
                                    })
                                    .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                                    .filter(|s| !s.is_empty());

                                results.push(WebImageResult {
                                    title,
                                    url,
                                    thumbnail: thumb_url,
                                    source_url,
                                    width,
                                    height,
                                    source: Some("wikimedia_commons".to_string()),
                                    description,
                                });
                                }
                            }
                        }
                    }

                }

                // Fallback: if total_available wasn't set, use results count
                // Runs outside pages block to handle edge case where API
                // returns searchinfo but no pages object
                if total_available == 0 {
                    total_available = results.len();
                }
            }
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => {
            tracing::warn!("Wikimedia Commons API: unexpected cached response in bypass mode");
        }
        Err(e) => {
            tracing::warn!("Wikimedia Commons API error: {}", e);
        }
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let count = results.len();

    Ok(WebImageSearchOutput {
        results,
        count,
        meta: WebSearchMeta {
            provider: "wikimedia_commons".to_string(),
            query: input.query.clone(),
            engines_used: vec!["wikimedia_commons".to_string()],
            response_time_ms: elapsed_ms,
            total_results: total_available.max(count),
            scored: Some(false),
        },
    })
}

// ─── DuckDuckGo Search Engine ─────────────────────────────────

/// Parse DuckDuckGo HTML search results page.
/// Domain filtering is handled by web_search after dedup.
fn parse_duckduckgo_html(html: &str, max_results: usize) -> Vec<WebSearchResult> {
    let doc = scraper::Html::parse_document(html);
    let mut results = Vec::new();

    let selectors = [".result__body", ".web-result", ".results_links", ".result"];

    for selector_str in &selectors {
        if let Ok(sel) = scraper::Selector::parse(selector_str) {
            for element in doc.select(&sel).take(max_results * 2) {
                if let Ok(link_sel) = scraper::Selector::parse("a.result__a") {
                    if let Some(link_el) = element.select(&link_sel).next() {
                        let title = link_el.text().collect::<String>().trim().to_string();
                        let raw_href = link_el.attr("href").unwrap_or("").to_string();
                        let url = extract_ddg_redirect_url(&raw_href).unwrap_or(raw_href);

                        let snippet = scraper::Selector::parse(".result__snippet")
                            .ok()
                            .and_then(|s| element.select(&s).next())
                            .map(|el| el.text().collect::<String>().trim().to_string());

                        let domain = scraper::Selector::parse(".result__url")
                            .ok()
                            .and_then(|s| element.select(&s).next())
                            .map(|el| el.text().collect::<String>().trim().to_string())
                            .or_else(|| extract_domain(&url));

                        if !url.is_empty() && !title.is_empty() {
                            results.push(WebSearchResult {
                                title,
                                url,
                                content: snippet,
                                score: None,
                                highlights: None,
                                raw_content: None,
                                source: Some("duckduckgo".to_string()),
                                domain,
                                published_date: None,
                                favicon: None,
                chunks: None,
                            });
                        }
                    }
                }
            }
            if !results.is_empty() { break; }
        }
    }
    results.truncate(max_results);
    results
}

/// Search DuckDuckGo via Obscura headless browser. Returns (engine_name, results, answer).
pub(super) async fn search_duckduckgo(
    query: &str,
    max_results: usize,
    include_answer: bool,
    time_range: &str,
    obs_settings: &crate::types::ObscuraSettings,
    http: &HttpClient,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    if !obs_settings.enabled {
        return Err("Obscura not enabled".into());
    }

    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    // DDG supports &df= for date filtering: d(day), w(week), m(month), y(year)
    let df_param = match time_range {
        "day" => "&df=d",
        "week" => "&df=w",
        "month" => "&df=m",
        "year" => "&df=y",
        _ => "",
    };
    let search_url = format!("https://html.duckduckgo.com/html/?q={}", query_encoded) + df_param;

    let html = obscura
        .fetch_with_all_options(&search_url, "html", false, "load", false, None)
        .await
        .map_err(|e| format!("DDG search failed: {}", e))?;

    let results = parse_duckduckgo_html(&html, max_results);

    // Optionally fetch DDG Instant Answer API for a synthesized answer
    let answer = if include_answer {
        let ia_url = format!("https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1", query_encoded);
        match http.fetch(&ia_url, None, "bypass").await {
            Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
                serde_json::from_str::<serde_json::Value>(&resp.body_text)
                    .ok()
                    .and_then(|j| j["AbstractText"].as_str().map(|s| s.to_string()))
                    .filter(|s| !s.is_empty())
            }
            _ => None,
        }
    } else {
        None
    };

    Ok(("duckduckgo".to_string(), results, answer))
}




// ─── Wikipedia REST API Engine ────────────────────────────────

/// Search Wikipedia via their free REST API. Returns (engine_name, results, answer).
pub(super) async fn search_wikipedia(
    query: &str,
    max_results: usize,
    http: &HttpClient,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let search_url = format!(
        "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
        query_encoded
    );

    // Try the summary endpoint first (direct article lookup)
    let mut results = Vec::new();
    let mut answer = None;

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&search_url, None, "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(title) = json["title"].as_str() {
                let url = json["content_urls"]["desktop"]["page"].as_str().unwrap_or("").to_string();
                let extract = json["extract"].as_str().unwrap_or("").to_string();
                let thumbnail = json["thumbnail"]["source"].as_str().map(|s| s.to_string());

                if !url.is_empty() {
                    answer = Some(extract.clone());
                    results.push(WebSearchResult {
                        title: title.to_string(),
                        url,
                        content: Some(extract),
                        score: Some(1.0),
                        highlights: None,
                        raw_content: None,
                        source: Some("wikipedia".to_string()),
                        domain: Some("wikipedia.org".to_string()),
                        published_date: None,
                        favicon: thumbnail,
                chunks: None,
                    });
                }
            }
        }
    }

    // Also do a search for related articles
    let search_api_url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit={}",
        query_encoded, max_results
    );

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&search_api_url, None, "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(search_results) = json["query"]["search"].as_array() {
                for r in search_results.iter().take(max_results) {
                    let title = r["title"].as_str().unwrap_or("").to_string();
                    let snippet = r["snippet"].as_str()
                        .unwrap_or("")
                        .replace("<span class=\"searchmatch\">", "")
                        .replace("</span>", "")
                        .to_string();
                    let url = format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_"));

                    // Skip if we already have this URL from the summary endpoint
                    if results.iter().any(|existing| existing.url == url) { continue; }

                    results.push(WebSearchResult {
                        title,
                        url,
                        content: if snippet.is_empty() { None } else { Some(snippet) },
                        score: Some(0.8),
                        highlights: None,
                        raw_content: None,
                        source: Some("wikipedia".to_string()),
                        domain: Some("wikipedia.org".to_string()),
                        published_date: None,
                        favicon: None,
                chunks: None,
                    });
                }
            }
        }
    }

    Ok(("wikipedia".to_string(), results, answer))
}

// ─── GitHub Search API Engine ──────────────────────────────────

/// Search GitHub via their free REST API. Returns (engine_name, results, answer).
/// When topic="code", also searches for matching code files.
pub(super) async fn search_github(
    query: &str,
    max_results: usize,
    http: &HttpClient,
    topic: &str,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let search_url = format!(
        "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page={}",
        query_encoded, max_results.min(10)
    );

    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("Accept".to_string(), "application/vnd.github.v3+json".to_string());

    let mut results = Vec::new();

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&search_url, Some(&headers), "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(items) = json["items"].as_array() {
                for r in items.iter().take(max_results) {
                    let name = r["full_name"].as_str().unwrap_or("").to_string();
                    let description = r["description"].as_str().unwrap_or("").to_string();
                    let html_url = r["html_url"].as_str().unwrap_or("").to_string();
                    let stars = r["stargazers_count"].as_u64().unwrap_or(0);
                    let language = r["language"].as_str().unwrap_or("").to_string();
                    let updated = r["updated_at"].as_str().map(|s| s.to_string());

                    let content = format!(
                        "⭐ {} stars | 📝 {} | {}",
                        stars, language, description
                    );

                    results.push(WebSearchResult {
                        title: name,
                        url: html_url,
                        content: Some(content),
                        score: Some(stars as f64 / 100000.0).map(|s| s.min(1.0)),
                        highlights: None,
                        raw_content: None,
                        source: Some("github".to_string()),
                        domain: Some("github.com".to_string()),
                        published_date: updated,
                        favicon: None,
                chunks: None,
                    });
                }
            }
        }
    }

    // Also search code when topic is explicitly code-related
    if topic == "code" {
    let code_url = format!(
        "https://api.github.com/search/code?q={}&per_page={}",
        query_encoded, max_results.min(5)
    );

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&code_url, Some(&headers), "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(items) = json["items"].as_array() {
                for r in items.iter().take(5) {
                    let path = r["path"].as_str().unwrap_or("").to_string();
                    let repo = r["repository"]["full_name"].as_str().unwrap_or("").to_string();
                    let html_url = r["html_url"].as_str().unwrap_or("").to_string();
                    let score_val = r["score"].as_f64().unwrap_or(0.0);

                    let title = format!("{}/{}", repo, path);
                    let content = r["text_matches"].as_array()
                        .map(|tm| tm.iter().take(2).map(|m| m["fragment"].as_str().unwrap_or("")).collect::<Vec<_>>().join(" ... "))
                        .unwrap_or_default();

                    results.push(WebSearchResult {
                        title,
                        url: html_url,
                        content: if content.is_empty() { None } else { Some(content) },
                        score: Some(score_val),
                        highlights: None,
                        raw_content: None,
                        source: Some("github".to_string()),
                        domain: Some("github.com".to_string()),
                        published_date: None,
                        favicon: None,
                chunks: None,
                    });
                }
            }
        }
    }
    } // end if topic == "code"

    Ok(("github".to_string(), results, None))
}

// ─── Hacker News Engine (Algolia API) ─────────────────────────

/// Search Hacker News via Algolia's free API. Returns (engine_name, results, answer).
/// API: https://hn.algolia.com/api/v1/search?query={q}&tags=story&hitsPerPage={limit}
pub(super) async fn search_hackernews(
    query: &str,
    max_results: usize,
    http: &HttpClient,
    time_range: &str,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let hits = max_results.min(30);
    // Add time range filter via numericFilters (unix timestamp)
    let time_filter = chrono::Utc::now().timestamp() - match time_range {
        "day" => 86400,
        "week" => 604800,
        "month" => 2592000,
        "year" => 31536000,
        _ => 0,
    };
    let url = if time_filter > 0 {
        format!(
            "https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage={}&numericFilters=created_at_i>{}",
            query_encoded, hits, time_filter
        )
    } else {
        format!(
            "https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage={}",
            query_encoded, hits
        )
    };

    match http.fetch(&url, None, "bypass").await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("HN parse error: {}", e))?;

            let mut results = Vec::new();

            if let Some(hits_arr) = json["hits"].as_array() {
                for hit in hits_arr.iter().take(max_results) {
                    let title = hit["title"].as_str().unwrap_or("").to_string();
                    let url_str = hit["url"].as_str()
                        .unwrap_or(&format!("https://news.ycombinator.com/item?id={}", hit["objectID"].as_str().unwrap_or("")))
                        .to_string();
                    let author = hit["author"].as_str().unwrap_or("").to_string();
                    let points = hit["points"].as_u64().unwrap_or(0);
                    let num_comments = hit["num_comments"].as_u64().unwrap_or(0);
                    let created_at = hit["created_at"].as_str().unwrap_or("").to_string();

                    let content = format!(
                        "⬆️ {} points | 💬 {} comments | by {}",
                        points, num_comments, author
                    );

                    results.push(WebSearchResult {
                        title,
                        url: url_str,
                        content: Some(content),
                        score: Some((points as f64 / 500.0).min(1.0)),
                        highlights: None,
                        raw_content: None,
                        source: Some("hackernews".to_string()),
                        domain: Some("news.ycombinator.com".to_string()),
                        published_date: Some(created_at),
                        favicon: None,
                chunks: None,
                    });
                }
            }

            Ok(("hackernews".to_string(), results, None))
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => {
            // Bypass mode should never return Cached; treat as error
            Err("HN API: unexpected cached response in bypass mode".into())
        }
        Err(e) => Err(format!("HN API error: {}", e)),
    }
}

// ─── Stack Overflow Engine (StackExchange API) ────────────────

/// Search Stack Overflow via StackExchange API (free, 10K requests/day with API key).
/// Returns (engine_name, results, answer).
/// API: https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle={q}&site=stackoverflow
pub(super) async fn search_stackoverflow(
    query: &str,
    max_results: usize,
    http: &HttpClient,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let pagesize = max_results.min(20);
    // Use 'all' instead of 'intitle' to search title+body for better relevance
    let url = format!(
        "https://api.stackexchange.com/2.3/search/advanced?order=desc&sort=relevance&q={}&site=stackoverflow&pagesize={}&filter=withbody",
        query_encoded, pagesize
    );

    match http.fetch(&url, None, "bypass").await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("SO parse error: {}", e))?;

            let mut results = Vec::new();

            if let Some(items) = json["items"].as_array() {
                for item in items.iter().take(max_results) {
                    let title = item["title"].as_str().unwrap_or("").to_string();
                    let link = item["link"].as_str().unwrap_or("").to_string();
                    let score = item["score"].as_i64().unwrap_or(0);
                    let answer_count = item["answer_count"].as_u64().unwrap_or(0);
                    let is_answered = item["is_answered"].as_bool().unwrap_or(false);
                    let tags: Vec<String> = item["tags"].as_array()
                        .map(|arr| arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    let created = item["creation_date"].as_i64().unwrap_or(0);
                    let view_count = item["view_count"].as_u64().unwrap_or(0);

                    // Convert unix timestamp to ISO date
                    let date_str = if created > 0 {
                        let dt = chrono::DateTime::from_timestamp(created, 0)
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_default();
                        dt
                    } else {
                        String::new()
                    };

                    let answered_marker = if is_answered { "✅" } else { "⏳" };
                    let content = format!(
                        "{} {} score | {} answers | 👁️ {} views | tags: {}",
                        answered_marker, score, answer_count, view_count, tags.join(", ")
                    );

                    results.push(WebSearchResult {
                        title,
                        url: link,
                        content: Some(content),
                        score: Some((score as f64 / 100.0).clamp(0.0, 1.0)),
                        highlights: None,
                        raw_content: None,
                        source: Some("stackoverflow".to_string()),
                        domain: Some("stackoverflow.com".to_string()),
                        published_date: if date_str.is_empty() { None } else { Some(date_str) },
                        favicon: None,
                chunks: None,
                    });
                }
            }

            Ok(("stackoverflow".to_string(), results, None))
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => {
            // Bypass mode should never return Cached; treat as error
            Err("SO API: unexpected cached response in bypass mode".into())
        }
        Err(e) => Err(format!("SO API error: {}", e)),
    }
}
