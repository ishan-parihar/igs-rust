use crate::http::{FetchOutcome, HttpClient};
use crate::types::{NewsItem, Source};
use anyhow::Result;
use chrono::Utc;
use feed_rs::parser;
use sha2::{Digest, Sha256};

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

                let now = Utc::now().to_rfc3339();
                let item_id = make_item_id(&title, &link, &now, &source.id);

                items.push(NewsItem {
                    id: item_id,
                    title,
                    link,
                    pub_date: now,
                    source_name: source_name.clone(),
                    pool_id: pool_id.clone(),
                    content_snippet,
                    author: None,
                    media_url: extract_attr(&element, "img", "src"),
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
            NewsItem {
                id: item_id, title, link, pub_date,
                source_name: source_name.clone(), pool_id: pool_id.clone(),
                content_snippet: strip_html_tags(&content).chars().take(600).collect(),
                author: None, media_url: None,
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
        items.push(NewsItem {
            id: make_item_id(&display_title, &link, &pub_date, &source.id),
            title: display_title, link, pub_date,
            source_name: source_name.clone(), pool_id: pool_id.clone(),
            content_snippet: desc, author: None, media_url: None,
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
                    return Some(d.and_hms_opt(0,0,0).unwrap().and_utc().to_rfc3339());
                }
                if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%b %d, %Y") {
                    return Some(d.and_hms_opt(0,0,0).unwrap().and_utc().to_rfc3339());
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
                    });
                }
            }
            if !items.is_empty() { break; }
        }
    }
    items
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
            if kw.as_array().is_some_and(|arr| arr.first().is_some_and(|v| v.is_array())) {
                // Already clustered: [[...], [...]]
                kw.as_array()
                    .unwrap()
                    .iter()
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
                vec![kw
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_lowercase())
                    .collect()]
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
                chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .or_else(|_| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map(|nd| nd.and_hms_opt(0, 0, 0).unwrap()))
                    .ok()
                    .map(|ndt| ndt.and_utc().timestamp_millis())
            })
        })
        .unwrap_or(i64::MIN);

    let e = end
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok().map(|dt| dt.timestamp_millis()))
        .or_else(|| {
            end.and_then(|d| {
                chrono::NaiveDateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .or_else(|_| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map(|nd| nd.and_hms_opt(23, 59, 59).unwrap()))
                    .ok()
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
