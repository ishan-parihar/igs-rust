use crate::http::{FetchOutcome, HttpClient};
use crate::types::{NewsItem, Source};
use anyhow::Result;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use feed_rs::parser;
use sha2::{Digest, Sha256};

pub fn calculate_freshness(pub_date: &str) -> f64 {
    let now = Utc::now();
    let parsed = DateTime::parse_from_rfc3339(pub_date)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            NaiveDateTime::parse_from_str(pub_date, "%Y-%m-%dT%H:%M:%S%.fZ")
                .ok()
                .map(|ndt| ndt.and_utc())
        })
        .or_else(|| {
            NaiveDate::parse_from_str(pub_date, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|ndt| ndt.and_utc())
        });

    match parsed {
        Some(dt) => {
            let hours = (now - dt).num_hours().max(0) as f64;
            100.0 * (-hours / 168.0).exp()
        }
        None => 0.0,
    }
}

pub fn parse_date_with_confidence(raw: &str) -> (String, String) {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return (dt.with_timezone(&Utc).to_rfc3339(), "high".to_string());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.fZ") {
        return (dt.and_utc().to_rfc3339(), "medium".to_string());
    }
    if let Ok(dt) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        if let Some(ndt) = dt.and_hms_opt(0, 0, 0) {
            return (ndt.and_utc().to_rfc3339(), "medium".to_string());
        }
    }
    for fmt in &[
        "%B %d, %Y",
        "%b %d, %Y",
        "%d %B %Y",
        "%d %b %Y",
        "%Y-%m-%d",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(raw, fmt) {
            return (dt.and_utc().to_rfc3339(), "medium".to_string());
        }
    }
    (raw.to_string(), "low".to_string())
}

/// Generate a stable ID for a news item
pub fn make_item_id(title: &str, link: &str, pub_date: &str, source_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}|{}|{}|{}", title, link, pub_date, source_id).as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..12])
}

/// Parse an RSS/Atom feed from a source
pub async fn parse_rss(source: &Source, xml_body: &str) -> Vec<NewsItem> {
    let feed = match parser::parse(xml_body.as_bytes()) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("RSS parse failed for {}: {}", source.id, e);
            return vec![];
        }
    };

    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());
    let now = Utc::now().to_rfc3339();

    feed.entries
        .iter()
        .map(|entry| {
            let title = entry
                .title
                .as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| "Untitled".to_string());

            let link = entry
                .links
                .first()
                .map(|l| l.href.clone())
                .or_else(|| {
                    let id = &entry.id;
                    if id.starts_with("http") {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let pub_date = entry
                .published
                .or(entry.updated)
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(|| now.clone());

            let content_snippet = entry
                .summary
                .as_ref()
                .map(|s| s.content.clone())
                .or_else(|| {
                    entry
                        .media
                        .first()
                        .and_then(|m| m.description.as_ref().map(|d| d.content.clone()))
                })
                .unwrap_or_default();

            // Strip HTML from snippet
            let content_snippet = strip_html_tags(&content_snippet);
            let content_snippet = content_snippet.chars().take(600).collect::<String>();

            let author = entry.authors.first().map(|a| a.name.clone()).filter(|n| !n.is_empty());

            let media_url = entry
                .media
                .first()
                .and_then(|m| m.content.first())
                .and_then(|c| c.url.as_ref())
                .map(|u| u.to_string());

            let item_id = make_item_id(
                &title,
                &link,
                &pub_date,
                &source.id,
            );

            let freshness = calculate_freshness(&pub_date);
            NewsItem {
                id: item_id,
                title,
                link,
                pub_date,
                source_name: source_name.clone(),
                pool_id: pool_id.clone(),
                content_snippet,
                author,
                media_url,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(freshness),
            }
        })
        .collect()
}

/// Parse content from a source based on its parser type
pub async fn parse_by_source(
    source: &Source,
    http: &HttpClient,
    cache_mode: &str,
    override_url: Option<&str>,
) -> Result<Vec<NewsItem>> {
    let url = override_url.unwrap_or_else(|| {
        // If source has parser_config.list_url, use that
        source.parser_config.as_ref()
            .and_then(|c| c.list_url.as_deref())
            .unwrap_or(&source.url)
    });
    let extra_headers = source.headers.as_ref();

    match source.platform.as_deref() {
        Some("reddit") | Some("twitter") => {
            return Ok(vec![]);
        }
        _ => {}
    }

    let outcome = http
        .fetch(url, extra_headers, cache_mode)
        .await?;

    match outcome {
        FetchOutcome::Cached(entry) => {
            Ok(entry.items)
        }
        FetchOutcome::Response(resp, etag, last_modified) => {
            let body = &resp.body_text;
            let items = match source.parser.as_deref() {
                Some("ofac") => {
                    parse_ofac(source, body).await
                }
                Some("ussf_cfc") => {
                    parse_generic_html(source, body).await
                }
                Some("who_dons") => {
                    // WHO DONS returns JSON disease outbreak news
                    parse_who_dons(source, body).await
                }
                Some("newslaundry") => {
                    // Newslaundry returns JSON-in-script
                    parse_newslaundry(source, body).await
                }
                Some("semantic_scholar") => {
                    parse_json_feed(source, body).await
                }
                Some("generic_html") => {
                    parse_generic_html(source, body).await
                }
                Some("hackernews") => {
                    parse_hackernews(source, body).await
                }
                Some("youtube") => {
                    parse_youtube_channel(source, body).await
                }
                Some("github") => {
                    parse_github(source, body).await
                }
                Some("bluesky") => {
                    parse_bluesky(source, body).await
                }
                _ => {
                    // Auto-detect
                    let body_trimmed = body.trim();
                    let ctype = resp
                        .headers
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_lowercase();
                    if ctype.contains("xml") || ctype.contains("rss") || ctype.contains("atom")
                        || body_trimmed.starts_with("<?xml") || body_trimmed.starts_with("<rss") || body_trimmed.starts_with("<feed")
                    {
                        parse_rss(source, body).await
                    } else if body_trimmed.starts_with('{') || body_trimmed.starts_with('[') {
                        parse_json_feed(source, body).await
                    } else {
                        parse_generic_html(source, body).await
                    }
                }
            };

            http.write_cache(url, items.clone(), etag, last_modified)
                .await?;

            Ok(items)
        }
    }
}

/// Minimal generic HTML parser using scraper crate
async fn parse_generic_html(source: &Source, html: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

    // Build selector list: from parser_config first, then fallback generics
    let mut selectors: Vec<String> = Vec::new();
    if let Some(ref cfg) = source.parser_config {
        if !cfg.selectors.as_ref().is_none_or(|s| s.item.is_empty()) {
            if let Some(ref sel) = cfg.selectors {
                selectors.push(sel.item.clone());
            }
        }
    }
    if selectors.is_empty() {
        selectors = vec![
            "article".into(),
            "div.article".into(),
            "div.post".into(),
            "div.entry".into(),
            "li".into(),
            "tr".into(),
            ".item".into(),
            ".story".into(),
            "[class*=article]".into(),
            "[class*=post]".into(),
            "[class*=story]".into(),
            "div.cmp-teaser".into(),
            "tr.circular-item".into(),
        ];
    }

    // Build title/link selectors from config if available
    let title_sub = source.parser_config.as_ref()
        .and_then(|c| c.selectors.as_ref())
        .and_then(|s| s.title.as_deref())
        .unwrap_or("h2 a, h3 a, h2, h3, .title, .headline, a[href]");

    let link_sel = source.parser_config.as_ref()
        .and_then(|c| c.selectors.as_ref())
        .and_then(|s| s.link.as_deref())
        .unwrap_or("a[href]");

    let document = scraper::Html::parse_document(html);
    let mut items = Vec::new();

    for sel_str in &selectors {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            for element in document.select(&sel) {
                let title = extract_first_text(&element, title_sub)
                    .or_else(|| {
                        element
                            .text()
                            .collect::<String>()
                            .split('\n')
                            .find(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string())
                    })
                    .unwrap_or_default();

                if title.is_empty() {
                    continue;
                }

                let link = extract_attr(&element, link_sel, "href")
                    .or_else(|| element.attr("href").map(|s| s.to_string()))
                    .unwrap_or_default();

                let link = if link.starts_with('/') {
                    let base = source.url.trim_end_matches('/').to_string();
                    format!("{}{}", base, link)
                } else {
                    link
                };

                let content_snippet = element
                    .text()
                    .collect::<String>()
                    .split_whitespace()
                    .take(100)
                    .collect::<Vec<_>>()
                    .join(" ");

                let date_sel = source.parser_config.as_ref()
                    .and_then(|c| c.selectors.as_ref())
                    .and_then(|s| s.date.as_deref());

                let (pub_date, date_confidence) = if let Some(sel_str) = date_sel {
                    if let Ok(_sel) = scraper::Selector::parse(sel_str) {
                        extract_first_text(&element, sel_str)
                            .map(|raw| {
                                let (parsed, conf) = parse_date_with_confidence(&raw);
                                (parsed, Some(conf))
                            })
                            .unwrap_or_else(|| {
                                let now = Utc::now().to_rfc3339();
                                (now, Some("low".to_string()))
                            })
                    } else {
                        let now = Utc::now().to_rfc3339();
                        (now, Some("low".to_string()))
                    }
                } else {
                    let now = Utc::now().to_rfc3339();
                    (now, Some("low".to_string()))
                };

                let freshness_score = Some(calculate_freshness(&pub_date));
                let item_id = make_item_id(&title, &link, &pub_date, &source.id);

                items.push(NewsItem {
                    id: item_id,
                    title,
                    link,
                    pub_date,
                    source_name: source_name.clone(),
                    pool_id: pool_id.clone(),
                    content_snippet,
                    author: None,
                    media_url: extract_attr(&element, "img", "src"),
                    date_confidence,
                    freshness_score,
                });

                // Limit per source
                if items.len() >= 50 {
                    break;
                }
            }
        }
        if !items.is_empty() {
            break;
        }
    }

    items
}

fn extract_first_text(element: &scraper::ElementRef, selector: &str) -> Option<String> {
    if let Ok(sel) = scraper::Selector::parse(selector) {
        if let Some(inner) = element.select(&sel).next() {
            let text = inner.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn extract_attr(element: &scraper::ElementRef, selector: &str, attr: &str) -> Option<String> {
    if let Ok(sel) = scraper::Selector::parse(selector) {
        if let Some(inner) = element.select(&sel).next() {
            if let Some(v) = inner.attr(attr) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Minimal JSON feed parser
async fn parse_json_feed(source: &Source, body: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());
    let now = Utc::now().to_rfc3339();

    // Try to parse as JSON feed (version 1)
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        let items_val: Option<&serde_json::Value> = if let Some(items) = val.get("items") {
            Some(items)
        } else if val.is_array() {
            Some(&val)
        } else {
            None
        };

        if let Some(items) = items_val {
            if let Some(arr) = items.as_array() {
                return arr
                    .iter()
                    .map(|item| {
                        let title = item
                            .get("title")
                            .and_then(|t| t.as_str())
                            .unwrap_or("Untitled");
                        let link = item
                            .get("url")
                            .or_else(|| item.get("external_url"))
                            .or_else(|| item.get("id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let pub_date = item
                            .get("date_published")
                            .and_then(|d| d.as_str())
                            .unwrap_or(&now);
                        let content = item
                            .get("content_text")
                            .or_else(|| item.get("summary"))
                            .or_else(|| item.get("content_html"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("");

                        let item_id = make_item_id(title, link, pub_date, &source.id);

                        NewsItem {
                            id: item_id,
                            title: title.to_string(),
                            link: link.to_string(),
                            pub_date: pub_date.to_string(),
                            source_name: source_name.clone(),
                            pool_id: pool_id.clone(),
                            content_snippet: strip_html_tags(content)
                                .chars()
                                .take(600)
                                .collect(),
                            author: item
                                .get("authors")
                                .and_then(|a| a.as_array())
                                .and_then(|a| a.first())
                                .and_then(|a| a.get("name"))
                                .and_then(|n| n.as_str())
                                .map(|s| s.to_string()),
                            media_url: item
                                .get("image")
                                .and_then(|i| i.as_str())
                                .map(|s| s.to_string()),
                            date_confidence: Some("high".to_string()),
                            freshness_score: Some(calculate_freshness(pub_date)),
                        }
                    })
                    .collect();
            }
        }
    }

    vec![]
}

/// Parse WHO Disease Outbreak News JSON (API returns {"value": [...]})
async fn parse_who_dons(source: &Source, body: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());
    let base_url = "https://www.who.int/emergencies/disease-outbreak-news/item/";

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
        // Check for {"value": [...]} format or bare array
        let items = if let Some(v) = val.get("value").and_then(|v| v.as_array()) {
            v.clone()
        } else if let Some(arr) = val.as_array() {
            arr.clone()
        } else { vec![] };
        return items.into_iter().map(|item| {
            let title = item.get("UseOverrideTitle").and_then(|u| u.as_bool()).unwrap_or(false)
                .then(|| item.get("OverrideTitle").and_then(|t| t.as_str()).unwrap_or(""))
                .or_else(|| item.get("Title").and_then(|t| t.as_str()))
                .unwrap_or("Untitled").to_string();
            let slug = item.get("UrlName").or_else(|| item.get("ItemDefaultUrl"))
                .and_then(|v| v.as_str()).unwrap_or("");
            let link = format!("{}{}", base_url, slug.trim_start_matches('/'));
            let pub_raw = item.get("PublicationDateAndTime")
                .or_else(|| item.get("PublicationDate"))
                .or_else(|| item.get("DateCreated"))
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let pub_date = chrono::DateTime::parse_from_rfc3339(pub_raw)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
            let content = [
                item.get("Summary").and_then(|v| v.as_str()),
                item.get("Overview").and_then(|v| v.as_str()),
                item.get("Assessment").and_then(|v| v.as_str()),
            ].iter().filter_map(|&s| s).collect::<Vec<_>>().join(" ");
            let item_id = make_item_id(&title, &link, &pub_date, &source.id);
            let freshness = calculate_freshness(&pub_date);
            NewsItem {
                id: item_id, title, link, pub_date,
                source_name: source_name.clone(), pool_id: pool_id.clone(),
                content_snippet: strip_html_tags(&content).chars().take(600).collect(),
                author: None, media_url: None,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(freshness),
            }
        }).collect();
    }
    vec![]
}

/// Parse OFAC Recent Actions using HTML block-split (same approach as TS version)
async fn parse_ofac(source: &Source, body: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());
    let base_url = "https://ofac.treasury.gov";
    let mut items = Vec::new();

    for block in body.split("<div class=\"margin-bottom-4 search-result views-row\">").skip(1) {
        // Manual <a href="...">text</a> extraction
        let mut links: Vec<(String, String)> = Vec::new();
        let mut pos = 0;
        while pos < block.len() {
            if let Some(start) = block[pos..].find("<a href=\"")
                .or_else(|| block[pos..].find("<a href='"))
            {
                let abs = pos + start;
                let href_begin = abs + 9;
                let q = block.as_bytes().get(href_begin.saturating_sub(1)).copied().unwrap_or(b'"');
                let quote_char = if q == b'\"' { '"' } else { '\'' };
                if let Some(href_end) = block[href_begin..].find(quote_char) {
                    let href = &block[href_begin..href_begin + href_end];
                    let after = href_begin + href_end + 1;
                    if let Some(close) = block[after..].find("</a>") {
                        let text = strip_html_tags(&block[after..after + close]).trim().to_string();
                        links.push((href.to_string(), text));
                        pos = after + close + 4;
                        continue;
                    }
                }
            }
            break;
        }

        if links.is_empty() { continue; }

        let (raw_link, raw_title) = &links[0];
        let link = if raw_link.starts_with("http") {
            raw_link.clone()
        } else if raw_link.starts_with('/') {
            format!("{}{}", base_url, raw_link)
        } else {
            format!("{}/{}", base_url, raw_link)
        };
        if raw_title.is_empty() { continue; }

        let category = if links.len() >= 2 {
            links[1].1.clone()
        } else { "Recent Action".to_string() };

        let pub_date = find_date_prefix(block).unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let desc = strip_html_tags(block).chars().take(600).collect::<String>();
        let display_title = format!("{} — {}", raw_title, category);
        let freshness = calculate_freshness(&pub_date);
        items.push(NewsItem {
            id: make_item_id(&display_title, &link, &pub_date, &source.id),
            title: display_title, link, pub_date,
            source_name: source_name.clone(), pool_id: pool_id.clone(),
            content_snippet: desc, author: None, media_url: None,
            date_confidence: Some("medium".to_string()),
            freshness_score: Some(freshness),
        });
    }
    items
}

fn find_date_prefix(text: &str) -> Option<String> {
    let months = ["January","February","March","April","May","June",
        "July","August","September","October","November","December"];
    for m in &months {
        if let Some(pos) = text.find(m) {
            let chunk = &text[pos..text.len().min(pos+50)];
            if let Some(end) = chunk.find(" - ") {
                let date_str = chunk[..end].trim();
                if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%B %d, %Y") {
                    return d.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().to_rfc3339());
                }
                if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%b %d, %Y") {
                    return d.and_hms_opt(0, 0, 0).map(|ndt| ndt.and_utc().to_rfc3339());
                }
            }
        }
    }
    None
}

/// Parse Newslaundry (JSON data embedded in script tags)
async fn parse_newslaundry(source: &Source, body: &str) -> Vec<NewsItem> {
    // Look specifically for <script id="static-page"> (sync helper)
    if let Some(val) = extract_script_by_id(body, "static-page") {
        let raw = extract_items_from_json(&val);
        if !raw.is_empty() {
            return raw.into_iter().map(|it| {
                NewsItem {
                    id: make_item_id(&it.title, &it.link, &it.pub_date, &source.id),
                    ..it
                }
            }).collect();
        }
    }

    // Fallback: try generic HTML
    parse_generic_html(source, body).await
}

/// Sync: extract JSON from a specific script tag by id (no Send issues)
fn extract_script_by_id(body: &str, script_id: &str) -> Option<serde_json::Value> {
    let doc = scraper::Html::parse_document(body);
    let sel = scraper::Selector::parse(&format!("script#{}", script_id)).ok()?;
    for el in doc.select(&sel) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&el.text().collect::<String>()) {
            return Some(val);
        }
    }
    None
}

fn extract_items_from_json(val: &serde_json::Value) -> Vec<NewsItem> {
    let mut items = Vec::new();
    let now = Utc::now().to_rfc3339();

    // Navigate common JSON structures to find article-like objects
    let candidates = [
        val.get("props").and_then(|p| p.get("pageProps")).and_then(|p| p.get("articles")),
        val.get("props").and_then(|p| p.get("pageProps")).and_then(|p| p.get("posts")),
        val.get("props").and_then(|p| p.get("pageProps")).and_then(|p| p.get("data")),
        val.get("props").and_then(|p| p.get("pageProps")).and_then(|p| p.get("news")),
        val.get("articles"),
        val.get("posts"),
        val.get("data"),
        val.get("items"),
    ];

    for candidate in candidates.iter().flatten() {
        if let Some(arr) = candidate.as_array() {
            for entry in arr {
                if let Some(obj) = entry.as_object() {
                    let title = obj.get("title")
                        .or_else(|| obj.get("headline"))
                        .or_else(|| obj.get("name"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    if title.is_empty() { continue; }
                    let link = obj.get("url")
                        .or_else(|| obj.get("link"))
                        .or_else(|| obj.get("slug"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let link = if link.starts_with('/') {
                        format!("https://www.newslaundry.com{}", link)
                    } else { link.to_string() };
                    let pub_date = obj.get("published_at")
                        .or_else(|| obj.get("date"))
                        .or_else(|| obj.get("createdAt"))
                        .and_then(|d| d.as_str())
                        .unwrap_or(&now);
                    let content = obj.get("excerpt")
                        .or_else(|| obj.get("summary"))
                        .or_else(|| obj.get("description"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    items.push(NewsItem {
                        id: String::new(), title: title.to_string(), link,
                        pub_date: pub_date.to_string(),
                        source_name: String::new(), pool_id: String::new(),
                        content_snippet: content.chars().take(600).collect(),
                        author: None, media_url: None,
                        date_confidence: Some("medium".to_string()),
                        freshness_score: Some(calculate_freshness(pub_date)),
                    });
                }
            }
            if !items.is_empty() { break; }
        }
    }
    items
}

async fn parse_hackernews(source: &Source, body: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

    #[derive(serde::Deserialize)]
    struct HnHit {
        title: Option<String>,
        url: Option<String>,
        created_at: Option<String>,
        author: Option<String>,
        points: Option<i64>,
        num_comments: Option<i64>,
        object_id: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct HnResponse {
        hits: Vec<HnHit>,
    }

    let resp: HnResponse = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("HN parse failed for {}: {}", source.id, e);
            return vec![];
        }
    };

    resp.hits
        .into_iter()
        .filter_map(|hit| {
            let title = hit.title?;
            let item_url = hit.url.unwrap_or_else(|| {
                format!("https://news.ycombinator.com/item?id={}", hit.object_id.as_deref().unwrap_or(""))
            });
            let pub_date = hit.created_at
                .map(|d| parse_date_with_confidence(&d).0)
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let snippet = format!(
                "Points: {} | Comments: {}",
                hit.points.unwrap_or(0),
                hit.num_comments.unwrap_or(0)
            );
            let item_id = make_item_id(&title, &item_url, &pub_date, &source.id);
            let freshness = calculate_freshness(&pub_date);

            Some(NewsItem {
                id: item_id,
                title,
                link: item_url,
                pub_date,
                source_name: source_name.clone(),
                pool_id: pool_id.clone(),
                content_snippet: snippet,
                author: hit.author,
                media_url: None,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(freshness),
            })
        })
        .collect()
}

async fn parse_youtube_channel(source: &Source, xml_body: &str) -> Vec<NewsItem> {
    let feed = match feed_rs::parser::parse(xml_body.as_bytes()) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("YouTube RSS parse failed for {}: {}", source.id, e);
            return vec![];
        }
    };

    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

    feed.entries
        .iter()
        .map(|entry| {
            let title = entry.title.as_ref().map(|t| t.content.clone()).unwrap_or_else(|| "Untitled".to_string());
            let link = entry.links.first().map(|l| l.href.clone()).unwrap_or_default();
            let pub_date = entry.published
                .or(entry.updated)
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let content_snippet = entry.summary.as_ref().map(|s| s.content.clone()).unwrap_or_default();
            let content_snippet = strip_html_tags(&content_snippet).chars().take(600).collect::<String>();
            let author = entry.authors.first().map(|a| a.name.clone()).filter(|n| !n.is_empty());

            let media_url = entry.media.iter()
                .flat_map(|m| m.thumbnails.iter())
                .next()
                .map(|t| t.image.uri.clone())
                .or_else(|| entry.media.iter()
                    .flat_map(|m| m.content.iter())
                    .next()
                    .and_then(|c| c.url.as_ref().map(|u| u.to_string())));

            let item_id = make_item_id(&title, &link, &pub_date, &source.id);
            let freshness = calculate_freshness(&pub_date);

            NewsItem {
                id: item_id,
                title,
                link,
                pub_date,
                source_name: source_name.clone(),
                pool_id: pool_id.clone(),
                content_snippet,
                author,
                media_url,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(freshness),
            }
        })
        .collect()
}

async fn parse_github(source: &Source, body: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

    #[derive(serde::Deserialize)]
    struct GhAuthor {
        login: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct GhRelease {
        name: Option<String>,
        tag_name: Option<String>,
        html_url: Option<String>,
        published_at: Option<String>,
        body: Option<String>,
        author: Option<GhAuthor>,
    }
    #[derive(serde::Deserialize)]
    struct GhRepo {
        full_name: Option<String>,
        html_url: Option<String>,
        description: Option<String>,
        updated_at: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct GhSearchResult {
        items: Vec<GhRepo>,
    }

    if let Ok(releases) = serde_json::from_str::<Vec<GhRelease>>(body) {
        return releases.into_iter().filter_map(|rel| {
            let title = rel.name.or(rel.tag_name)?;
            let link = rel.html_url.unwrap_or_default();
            let pub_date = rel.published_at
                .map(|d| parse_date_with_confidence(&d).0)
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let content = rel.body.unwrap_or_default();
            let item_id = make_item_id(&title, &link, &pub_date, &source.id);
            let freshness = calculate_freshness(&pub_date);

            Some(NewsItem {
                id: item_id,
                title,
                link,
                pub_date,
                source_name: source_name.clone(),
                pool_id: pool_id.clone(),
                content_snippet: strip_html_tags(&content).chars().take(600).collect(),
                author: rel.author.and_then(|a| a.login),
                media_url: None,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(freshness),
            })
        }).collect();
    }

    if let Ok(search) = serde_json::from_str::<GhSearchResult>(body) {
        return search.items.into_iter().filter_map(|repo| {
            let name = repo.full_name?;
            let link = repo.html_url.unwrap_or_default();
            let pub_date = repo.updated_at
                .map(|d| parse_date_with_confidence(&d).0)
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let content = repo.description.unwrap_or_default();
            let item_id = make_item_id(&name, &link, &pub_date, &source.id);
            let freshness = calculate_freshness(&pub_date);

            Some(NewsItem {
                id: item_id,
                title: name,
                link,
                pub_date,
                source_name: source_name.clone(),
                pool_id: pool_id.clone(),
                content_snippet: content,
                author: None,
                media_url: None,
                date_confidence: Some("medium".to_string()),
                freshness_score: Some(freshness),
            })
        }).collect();
    }

    vec![]
}

async fn parse_bluesky(source: &Source, body: &str) -> Vec<NewsItem> {
    let source_name = source.name.clone();
    let pool_id = source.pools.first().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

    #[derive(serde::Deserialize)]
    struct BskyAuthor {
        handle: Option<String>,
        display_name: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct BskyRecord {
        text: Option<String>,
        created_at: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct BskyPost {
        uri: Option<String>,
        _cid: Option<String>,
        record: Option<BskyRecord>,
        author: Option<BskyAuthor>,
        like_count: Option<i64>,
        reply_count: Option<i64>,
        _repost_count: Option<i64>,
    }
    #[derive(serde::Deserialize)]
    struct BskyResponse {
        posts: Vec<BskyPost>,
    }

    let resp: BskyResponse = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Bluesky parse failed for {}: {}", source.id, e);
            return vec![];
        }
    };

    resp.posts
        .into_iter()
        .filter_map(|post| {
            let record = post.record?;
            let text = record.text?;
            let created_at = record.created_at.unwrap_or_default();
            let pub_date = parse_date_with_confidence(&created_at).0;

            let link = post.uri.as_deref()
                .map(|uri| {
                    let rkey = uri.rsplit('/').next().unwrap_or("");
                    let handle = post.author.as_ref()
                        .and_then(|a| a.handle.as_deref())
                        .unwrap_or("unknown");
                    format!("https://bsky.app/profile/{}/post/{}", handle, rkey)
                })
                .unwrap_or_default();

            let author_name = post.author.as_ref()
                .and_then(|a| a.display_name.as_deref().or(a.handle.as_deref()))
                .map(|s| s.to_string());

            let item_id = make_item_id(&text, &link, &pub_date, &source.id);
            let freshness = calculate_freshness(&pub_date);

            let snippet = format!(
                "{}{}{}",
                text.chars().take(500).collect::<String>(),
                post.like_count.map(|l| format!(" | Likes: {}", l)).unwrap_or_default(),
                post.reply_count.map(|r| format!(" | Replies: {}", r)).unwrap_or_default(),
            );

            Some(NewsItem {
                id: item_id,
                title: text.chars().take(120).collect(),
                link,
                pub_date,
                source_name: source_name.clone(),
                pool_id: pool_id.clone(),
                content_snippet: snippet,
                author: author_name,
                media_url: None,
                date_confidence: Some("medium".to_string()),
                freshness_score: Some(freshness),
            })
        })
        .collect()
}

/// Strip HTML tags from a string
pub fn strip_html_tags(s: &str) -> String {
    let document = scraper::Html::parse_fragment(s);
    document.root_element().text().collect::<String>()
}

/// Filter news items by keywords (accepts both single keywords and keyword clusters)
pub fn filter_by_keywords(
    items: Vec<NewsItem>,
    keywords: Option<&serde_json::Value>,
    exclude_keywords: &[String],
    match_all: bool,
) -> Vec<NewsItem> {
    if keywords.is_none() && exclude_keywords.is_empty() {
        return items;
    }

    let exs: Vec<String> = exclude_keywords.iter().map(|k| k.to_lowercase()).collect();

    // Normalize keywords into clusters
    let clusters: Vec<Vec<String>> = match keywords {
        Some(kw) if kw.is_array() => {
            if let Some(arr) = kw.as_array() {
                if arr.first().is_some_and(|v| v.is_array()) {
                    // Already clustered: [[...], [...]]
                    arr.iter()
                        .map(|cluster| {
                            cluster
                                .as_array()
                                .unwrap_or(&vec![])
                                .iter()
                                .map(|v| v.as_str().unwrap_or("").to_lowercase())
                                .collect()
                        })
                        .collect()
                } else {
                    // Flat array: ["a", "b"]
                    vec![arr
                        .iter()
                        .map(|v| v.as_str().unwrap_or("").to_lowercase())
                        .collect()]
                }
            } else {
                vec![]
            }
        }
        _ => vec![],
    };

    items
        .into_iter()
        .filter(|it| {
            let text = format!(
                "{} {} {}",
                it.title.to_lowercase(),
                it.content_snippet.to_lowercase(),
                it.link.to_lowercase()
            );

            // Exclude check
            if !exs.is_empty() && exs.iter().any(|e| text.contains(e)) {
                return false;
            }

            if clusters.is_empty() {
                return true;
            }

            // For each cluster, at least one term must match
            let cluster_matches: Vec<bool> = clusters
                .iter()
                .map(|cluster| cluster.iter().any(|term| text.contains(term)))
                .collect();

            if match_all {
                cluster_matches.iter().all(|&m| m)
            } else {
                cluster_matches.iter().any(|&m| m)
            }
        })
        .collect()
}

/// Filter items by time range (ISO date strings)
pub fn filter_by_time(items: Vec<NewsItem>, start: Option<&str>, end: Option<&str>) -> Vec<NewsItem> {
    let s = start
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok().map(|dt| dt.timestamp_millis()))
        .or_else(|| {
            start.and_then(|d| {
                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .ok()
                    .and_then(|nd| nd.and_hms_opt(0, 0, 0))
                    .or_else(|| chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ").ok())
                    .map(|ndt| ndt.and_utc().timestamp_millis())
            })
        })
        .unwrap_or(i64::MIN);

    let e = end
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok().map(|dt| dt.timestamp_millis()))
        .or_else(|| {
            end.and_then(|d| {
                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .ok()
                    .and_then(|nd| nd.and_hms_opt(23, 59, 59))
                    .or_else(|| chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ").ok())
                    .map(|ndt| ndt.and_utc().timestamp_millis())
            })
        })
        .unwrap_or(i64::MAX);

    items
        .into_iter()
        .filter(|it| {
            let t = chrono::DateTime::parse_from_rfc3339(&it.pub_date)
                .ok()
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(0);
            t >= s && t <= e
        })
        .collect()
}

/// Batch similar news items together (deduplication)
pub fn batch_similar(items: Vec<NewsItem>, threshold: f64) -> Vec<NewsItem> {
    if items.len() <= 1 {
        return items;
    }

    let mut results = Vec::new();
    let mut used = std::collections::HashSet::new();

    for i in 0..items.len() {
        if used.contains(&i) {
            continue;
        }

        let mut cluster = vec![i];
        let words_i: std::collections::HashSet<String> = items[i]
            .title
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|w| w.to_string())
            .collect();

        for (j, item_j) in items.iter().enumerate().skip(i + 1) {
            if used.contains(&j) {
                continue;
            }

            let words_j: std::collections::HashSet<String> = item_j
                .title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .map(|w| w.to_string())
                .collect();

            let intersection = words_i.intersection(&words_j).count();
            let union = words_i.union(&words_j).count();
            let similarity = if union > 0 {
                intersection as f64 / union as f64
            } else {
                0.0
            };

            if similarity >= threshold {
                cluster.push(j);
            }
        }

        // Use the first item from each cluster
        results.push(items[cluster[0]].clone());
        for idx in cluster {
            used.insert(idx);
        }
    }

    results
}

pub fn cap_per_author(items: Vec<NewsItem>, max_per_author: usize) -> Vec<NewsItem> {
    let mut author_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    items.into_iter().filter(|item| {
        let author = item.author.as_deref().unwrap_or("unknown");
        let count = author_counts.entry(author.to_string()).or_insert(0);
        if *count < max_per_author {
            *count += 1;
            true
        } else {
            false
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_source() -> Source {
        Source {
            id: "test_source".to_string(),
            name: "Test Source".to_string(),
            source_type: "rss".to_string(),
            url: "https://example.com".to_string(),
            headers: None,
            parser: None,
            parser_config: None,
            pools: vec!["TEST_POOL".to_string()],
            countries: vec![],
            cities: vec![],
            domains: vec![],
            is_active: Some(true),
            platform: None,
            tier: None,
            rate_limit: None,
            source_category: None,
            weight: None,
            trust_score: None,
        }
    }

    fn make_test_item(title: &str, author: Option<&str>, pub_date: &str) -> NewsItem {
        NewsItem {
            id: String::new(),
            title: title.to_string(),
            link: "https://example.com/article".to_string(),
            pub_date: pub_date.to_string(),
            source_name: "Test".to_string(),
            pool_id: "TEST".to_string(),
            content_snippet: String::new(),
            author: author.map(|s| s.to_string()),
            media_url: None,
            date_confidence: None,
            freshness_score: None,
        }
    }

    // ── calculate_freshness ──────────────────────────────────────

    #[test]
    fn freshness_now_returns_near_100() {
        let now = Utc::now().to_rfc3339();
        let score = calculate_freshness(&now);
        assert!((score - 100.0).abs() < 0.01, "Expected ~100, got {}", score);
    }

    #[test]
    fn freshness_one_day_ago_decays_as_expected() {
        let day_ago = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        let score = calculate_freshness(&day_ago);
        let expected = 100.0 * (-24.0_f64 / 168.0).exp();
        assert!((score - expected).abs() < 0.5, "Expected ~{:.2}, got {}", expected, score);
    }

    #[test]
    fn freshness_one_week_ago_returns_about_36_8() {
        let week_ago = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let score = calculate_freshness(&week_ago);
        let expected = 100.0 * (-1.0_f64).exp();
        assert!((score - expected).abs() < 0.5, "Expected ~{:.2}, got {}", expected, score);
    }

    #[test]
    fn freshness_one_month_ago_decays_near_zero() {
        let month_ago = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let score = calculate_freshness(&month_ago);
        assert!(score < 5.0, "Expected near 0, got {}", score);
    }

    #[test]
    fn freshness_invalid_date_returns_zero() {
        assert_eq!(calculate_freshness("not-a-date"), 0.0);
    }

    #[test]
    fn freshness_empty_string_returns_zero() {
        assert_eq!(calculate_freshness(""), 0.0);
    }

    // ── parse_date_with_confidence ──────────────────────────────

    #[test]
    fn parse_rfc3339_datetime_returns_high_confidence() {
        let (parsed, confidence) = parse_date_with_confidence("2024-01-15T10:30:00+00:00");
        assert!(parsed.contains("2024-01-15T10:30:00"), "parsed={}", parsed);
        assert_eq!(confidence, "high");
    }

    #[test]
    fn parse_rfc3339_with_offset_converts_to_utc() {
        let (parsed, confidence) = parse_date_with_confidence("2024-06-01T12:00:00+05:30");
        // +05:30 offset means 06:30 UTC
        assert!(parsed.contains("T06:30:00"), "parsed={}", parsed);
        assert_eq!(confidence, "high");
    }

    #[test]
    fn parse_datetime_with_z_is_valid_rfc3339_high() {
        // "2024-01-15T10:30:00.000Z" is valid RFC3339 (Z suffix counts as timezone),
        // so it returns "high" from the RFC3339 branch, not the naive datetime path.
        let (parsed, confidence) = parse_date_with_confidence("2024-01-15T10:30:00.000Z");
        assert!(parsed.contains("2024-01-15T10:30:00"), "parsed={}", parsed);
        assert_eq!(confidence, "high");
    }

    #[test]
    fn parse_date_only_returns_medium_confidence() {
        let (parsed, confidence) = parse_date_with_confidence("2024-01-15");
        assert!(parsed.starts_with("2024-01-15"), "parsed={}", parsed);
        assert_eq!(confidence, "medium");
    }

    #[test]
    fn parse_named_date_without_time_falls_to_low() {
        // NaiveDateTime::parse_from_str requires time components, so "January 15, 2024"
        // falls through to "low" despite being a valid date string.
        let input = "January 15, 2024";
        let (parsed, confidence) = parse_date_with_confidence(input);
        assert_eq!(parsed, input);
        assert_eq!(confidence, "low");
    }

    #[test]
    fn parse_garbage_returns_low_confidence() {
        let input = "this is not a date at all";
        let (parsed, confidence) = parse_date_with_confidence(input);
        assert_eq!(parsed, input);
        assert_eq!(confidence, "low");
    }

    #[test]
    fn parse_empty_string_returns_low() {
        let (_parsed, confidence) = parse_date_with_confidence("");
        assert_eq!(confidence, "low");
    }

    // ── cap_per_author ──────────────────────────────────────────

    #[test]
    fn cap_limits_items_per_author() {
        let items = vec![
            make_test_item("A1", Some("alice"), "2024-01-01"),
            make_test_item("A2", Some("alice"), "2024-01-02"),
            make_test_item("A3", Some("alice"), "2024-01-03"),
            make_test_item("A4", Some("alice"), "2024-01-04"),
            make_test_item("A5", Some("alice"), "2024-01-05"),
            make_test_item("B1", Some("bob"), "2024-01-01"),
            make_test_item("B2", Some("bob"), "2024-01-02"),
        ];
        let result = cap_per_author(items, 2);
        assert_eq!(result.len(), 4);
        assert_eq!(
            result.iter().filter(|i| i.author.as_deref() == Some("alice")).count(),
            2
        );
        assert_eq!(
            result.iter().filter(|i| i.author.as_deref() == Some("bob")).count(),
            2
        );
    }

    #[test]
    fn cap_all_from_one_author_respects_limit() {
        let items = vec![
            make_test_item("A1", Some("alice"), "2024-01-01"),
            make_test_item("A2", Some("alice"), "2024-01-02"),
            make_test_item("A3", Some("alice"), "2024-01-03"),
        ];
        let result = cap_per_author(items, 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn cap_empty_input_returns_empty() {
        let result = cap_per_author(vec![], 5);
        assert!(result.is_empty());
    }

    #[test]
    fn cap_zero_max_returns_empty() {
        let items = vec![make_test_item("A1", Some("alice"), "2024-01-01")];
        let result = cap_per_author(items, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn cap_no_author_falls_back_to_unknown() {
        let items = vec![
            make_test_item("A1", None, "2024-01-01"),
            make_test_item("A2", None, "2024-01-02"),
            make_test_item("A3", None, "2024-01-03"),
        ];
        let result = cap_per_author(items, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn cap_under_limit_keeps_all_items() {
        let items = vec![
            make_test_item("A1", Some("alice"), "2024-01-01"),
            make_test_item("B1", Some("bob"), "2024-01-01"),
        ];
        let result = cap_per_author(items, 5);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn cap_preserves_order() {
        let items = vec![
            make_test_item("First", Some("alice"), "2024-01-01"),
            make_test_item("Second", Some("alice"), "2024-01-02"),
            make_test_item("Third", Some("alice"), "2024-01-03"),
        ];
        let result = cap_per_author(items, 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title, "First");
        assert_eq!(result[1].title, "Second");
    }

    // ── parse_hackernews ────────────────────────────────────────

    #[tokio::test]
    async fn hn_parses_valid_response() {
        let src = make_test_source();
        let body = r#"{
            "hits": [
                {
                    "title": "Rust 1.78 Released",
                    "url": "https://example.com/rust-1.78",
                    "created_at": "2024-05-01T12:00:00Z",
                    "author": "johndoe",
                    "points": 150,
                    "num_comments": 45,
                    "object_id": "12345"
                }
            ]
        }"#;
        let items = parse_hackernews(&src, body).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Rust 1.78 Released");
        assert!(items[0].link.contains("example.com/rust-1.78"));
        assert!(items[0].content_snippet.contains("Points: 150"));
        assert!(items[0].content_snippet.contains("Comments: 45"));
        assert_eq!(items[0].author.as_deref(), Some("johndoe"));
    }

    #[tokio::test]
    async fn hn_empty_hits_returns_empty() {
        let src = make_test_source();
        let body = r#"{"hits": []}"#;
        let items = parse_hackernews(&src, body).await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn hn_missing_title_filtered_out() {
        let src = make_test_source();
        let body = r#"{
            "hits": [
                {
                    "url": "https://example.com/no-title",
                    "created_at": "2024-05-01T12:00:00Z",
                    "object_id": "67890"
                }
            ]
        }"#;
        let items = parse_hackernews(&src, body).await;
        assert!(items.is_empty(), "items without title should be filtered out");
    }

    #[tokio::test]
    async fn hn_missing_url_uses_hackernews_fallback() {
        let src = make_test_source();
        let body = r#"{
            "hits": [
                {
                    "title": "Discussion Thread",
                    "created_at": "2024-05-01T12:00:00Z",
                    "author": "asker",
                    "object_id": "99999"
                }
            ]
        }"#;
        let items = parse_hackernews(&src, body).await;
        assert_eq!(items.len(), 1);
        assert!(items[0].link.contains("news.ycombinator.com/item?id=99999"));
    }

    #[tokio::test]
    async fn hn_invalid_json_returns_empty() {
        let src = make_test_source();
        let items = parse_hackernews(&src, "not json at all").await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn hn_missing_created_at_uses_current_time() {
        let src = make_test_source();
        let body = r#"{
            "hits": [
                {
                    "title": "Timeless Post",
                    "url": "https://example.com/timeless",
                    "object_id": "11111"
                }
            ]
        }"#;
        let items = parse_hackernews(&src, body).await;
        assert_eq!(items.len(), 1);
        let parsed = DateTime::parse_from_rfc3339(&items[0].pub_date);
        assert!(parsed.is_ok(), "pub_date should be a valid RFC3339: {}", items[0].pub_date);
    }

    // ── parse_youtube_channel ───────────────────────────────────

    #[tokio::test]
    async fn yt_parses_valid_atom_feed() {
        let src = make_test_source();
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <title>Hello Rust World</title>
                <link href="https://youtube.com/watch?v=abc123" rel="alternate"/>
                <published>2024-06-01T10:00:00+00:00</published>
                <summary>A great Rust video about systems programming</summary>
                <author>
                    <name>RustChannel</name>
                </author>
            </entry>
            <entry>
                <title>Async Rust Deep Dive</title>
                <link href="https://youtube.com/watch?v=def456" rel="alternate"/>
                <published>2024-06-05T14:30:00+00:00</published>
                <summary>Understanding async/await in Rust</summary>
                <author>
                    <name>RustChannel</name>
                </author>
            </entry>
        </feed>"#;
        let items = parse_youtube_channel(&src, xml).await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Hello Rust World");
        assert_eq!(items[1].title, "Async Rust Deep Dive");
        assert_eq!(items[0].author.as_deref(), Some("RustChannel"));
        assert_eq!(items[1].author.as_deref(), Some("RustChannel"));
        assert!(items[0].link.contains("youtube.com/watch?v=abc123"));
    }

    #[tokio::test]
    async fn yt_invalid_xml_returns_empty() {
        let src = make_test_source();
        let items = parse_youtube_channel(&src, "not xml at all").await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn yt_empty_feed_returns_empty() {
        let src = make_test_source();
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
        </feed>"#;
        let items = parse_youtube_channel(&src, xml).await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn yt_missing_title_uses_fallback() {
        let src = make_test_source();
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <entry>
                <link href="https://youtube.com/watch?v=no-title" rel="alternate"/>
                <published>2024-06-01T10:00:00+00:00</published>
            </entry>
        </feed>"#;
        let items = parse_youtube_channel(&src, xml).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Untitled");
    }

    // ── parse_github ────────────────────────────────────────────

    #[tokio::test]
    async fn gh_parses_releases_array() {
        let src = make_test_source();
        let body = r#"[
            {
                "name": "v1.0.0",
                "tag_name": "v1.0.0",
                "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0",
                "published_at": "2024-06-15T00:00:00Z",
                "body": "Initial release with cool features",
                "author": {"login": "releaser"}
            },
            {
                "name": "v1.1.0",
                "tag_name": "v1.1.0",
                "html_url": "https://github.com/owner/repo/releases/tag/v1.1.0",
                "published_at": "2024-07-01T12:00:00Z",
                "body": "Bug fixes and improvements",
                "author": {"login": "releaser"}
            }
        ]"#;
        let items = parse_github(&src, body).await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "v1.0.0");
        assert_eq!(items[1].title, "v1.1.0");
        assert_eq!(items[0].author.as_deref(), Some("releaser"));
    }

    #[tokio::test]
    async fn gh_parses_search_result() {
        let src = make_test_source();
        let body = r#"{
            "items": [
                {
                    "full_name": "owner/awesome-repo",
                    "html_url": "https://github.com/owner/awesome-repo",
                    "description": "An awesome Rust project",
                    "updated_at": "2024-06-20T12:00:00Z"
                },
                {
                    "full_name": "owner/another-repo",
                    "html_url": "https://github.com/owner/another-repo",
                    "description": "Another cool project",
                    "updated_at": "2024-06-25T08:00:00Z"
                }
            ]
        }"#;
        let items = parse_github(&src, body).await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "owner/awesome-repo");
        assert_eq!(items[1].title, "owner/another-repo");
    }

    #[tokio::test]
    async fn gh_release_without_name_and_tag_skipped() {
        let src = make_test_source();
        let body = r#"[
            {
                "html_url": "https://github.com/owner/repo/releases/tag/v1.0.0",
                "published_at": "2024-06-15T00:00:00Z"
            }
        ]"#;
        let items = parse_github(&src, body).await;
        assert!(items.is_empty(), "release without name or tag_name should be filtered out");
    }

    #[tokio::test]
    async fn gh_invalid_json_returns_empty() {
        let src = make_test_source();
        let items = parse_github(&src, "not json at all").await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn gh_empty_array_returns_empty() {
        let src = make_test_source();
        let items = parse_github(&src, "[]").await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn gh_empty_search_items_returns_empty() {
        let src = make_test_source();
        let body = r#"{"items": []}"#;
        let items = parse_github(&src, body).await;
        assert!(items.is_empty());
    }

    // ── parse_bluesky ───────────────────────────────────────────

    #[tokio::test]
    async fn bsky_parses_valid_response() {
        let src = make_test_source();
        let body = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:abc/app.bsky.feed.post/123xyz",
                    "record": {
                        "text": "Hello Bluesky! This is a test post about decentralized social media.",
                        "created_at": "2024-07-01T08:00:00.000Z"
                    },
                    "author": {
                        "handle": "testuser.bsky.social",
                        "display_name": "Test User"
                    },
                    "like_count": 42,
                    "reply_count": 5
                }
            ]
        }"#;
        let items = parse_bluesky(&src, body).await;
        assert_eq!(items.len(), 1);
        assert!(items[0].title.contains("Hello Bluesky"));
        assert_eq!(items[0].author.as_deref(), Some("Test User"));
        assert!(
            items[0].link.contains("bsky.app/profile/testuser.bsky.social/post/123xyz"),
            "link={}",
            items[0].link
        );
        assert!(items[0].content_snippet.contains("Likes: 42"));
        assert!(items[0].content_snippet.contains("Replies: 5"));
    }

    #[tokio::test]
    async fn bsky_empty_posts_returns_empty() {
        let src = make_test_source();
        let body = r#"{"posts": []}"#;
        let items = parse_bluesky(&src, body).await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn bsky_missing_record_filtered_out() {
        let src = make_test_source();
        let body = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:abc/app.bsky.feed.post/456xyz",
                    "author": {"handle": "nopost.bsky.social"}
                }
            ]
        }"#;
        let items = parse_bluesky(&src, body).await;
        assert!(items.is_empty(), "posts without record should be filtered out");
    }

    #[tokio::test]
    async fn bsky_invalid_json_returns_empty() {
        let src = make_test_source();
        let items = parse_bluesky(&src, "not json at all").await;
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn bsky_missing_author_uses_none() {
        let src = make_test_source();
        let body = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:def/app.bsky.feed.post/789abc",
                    "record": {
                        "text": "Anonymous post without author info",
                        "created_at": "2024-07-02T12:00:00.000Z"
                    }
                }
            ]
        }"#;
        let items = parse_bluesky(&src, body).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author, None);
    }

    #[tokio::test]
    async fn bsky_missing_text_filtered_out() {
        let src = make_test_source();
        let body = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:abc/app.bsky.feed.post/nope",
                    "record": {
                        "created_at": "2024-07-02T12:00:00.000Z"
                    },
                    "author": {"handle": "empty.bsky.social"}
                }
            ]
        }"#;
        let items = parse_bluesky(&src, body).await;
        assert!(items.is_empty(), "posts without text should be filtered out");
    }

    #[tokio::test]
    async fn bsky_multiple_posts_all_parsed() {
        let src = make_test_source();
        let body = r#"{
            "posts": [
                {
                    "uri": "at://did:plc:a/app.bsky.feed.post/1",
                    "record": {"text": "First post", "created_at": "2024-07-01T08:00:00.000Z"},
                    "author": {"handle": "user1.bsky.social"}
                },
                {
                    "uri": "at://did:plc:b/app.bsky.feed.post/2",
                    "record": {"text": "Second post", "created_at": "2024-07-01T09:00:00.000Z"},
                    "author": {"handle": "user2.bsky.social", "display_name": "User Two"}
                }
            ]
        }"#;
        let items = parse_bluesky(&src, body).await;
        assert_eq!(items.len(), 2);
        assert!(items[0].title.contains("First post"));
        assert!(items[1].title.contains("Second post"));
        assert_eq!(items[1].author.as_deref(), Some("User Two"));
    }
}
