use crate::parsers;
use crate::tools::helpers::urlencoding;
use crate::tools::types::*;
use crate::types::*;
use feed_rs::parser as feed_parser;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;

const REDDIT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

/// Build a dedicated reqwest Client for Reddit with browser-like headers.
/// Uses a separate client because the shared HttpClient sets a bot UA (`IGS/...`)
/// which Reddit blocks. reqwest merges client-level + request-level headers,
/// so we need a clean client with only the browser UA.
fn build_reddit_client() -> Client {
    Client::builder()
        .user_agent(REDDIT_USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("Failed to build Reddit HTTP client")
}

async fn reddit_fetch(client: &Client, url: &str, accept: &str) -> Result<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Accept", accept);
    headers.insert("Accept-Language", "en-US,en;q=0.9");

    let max_retries = 3;
    let mut last_err = String::new();

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_secs(2u64.pow(attempt as u32));
            tracing::info!("Reddit: retry {} after {}s for {}", attempt, delay.as_secs(), url);
            tokio::time::sleep(delay).await;
        }

        let mut req = client.get(url);
        for (k, v) in &headers {
            req = req.header(*k, *v);
        }

        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();

                if status == 429 {
                let retry_after = resp.headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(10);

                    tracing::warn!("Reddit: 429 rate-limited for {}, Retry-After: {}s", url, retry_after);
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    last_err = format!("429 rate-limited (Retry-After: {}s)", retry_after);
                    continue;
                }

                if status >= 400 {
                    return Err(format!("HTTP {} from {}", status, url));
                }

                return resp.text().await.map_err(|e| format!("Read error: {}", e));
            }
            Err(e) => {
                last_err = e.to_string();
                tracing::warn!("Reddit: request error for {} (attempt {}): {}", url, attempt, e);
            }
        }
    }

    Err(format!("Failed after {} retries: {}", max_retries + 1, last_err))
}

pub async fn reddit_search(input: RedditSearchInput) -> Result<RedditSearchOutput, String> {
    let sort = input.sort.as_deref().unwrap_or("relevance");
    let time = input.time.as_deref().unwrap_or("all");
    let limit = input.limit.unwrap_or(25).clamp(1, 100);

    let query_enc = urlencoding(&input.query);
    let client = build_reddit_client();
    let mut posts = Vec::new();

    let subreddits = input.subreddits.clone().unwrap_or_default();
    let search_urls = if subreddits.is_empty() {
        vec![format!(
            "https://old.reddit.com/search?q={}&sort={}&t={}&limit={}",
            query_enc, sort, time, limit
        )]
    } else {
        subreddits.iter().map(|sr| {
            format!(
                "https://old.reddit.com/r/{}/search?q={}&restrict_sr=on&sort={}&t={}&limit={}",
                urlencoding(sr), query_enc, sort, time, limit
            )
        }).collect()
    };

    for (idx, url) in search_urls.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        let body = match reddit_fetch(&client, url, "text/html").await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Reddit search failed for {}: {}", url, e);
                continue;
            }
        };

        let document = scraper::Html::parse_document(&body);
        let selector = scraper::Selector::parse("div.search-result.search-result-link").unwrap();

        for result in document.select(&selector).take(limit as usize) {
            let (title, link) = if let Some(a) = result.select(&scraper::Selector::parse("a.search-title").unwrap()).next() {
                let title = parsers::strip_html_tags(&a.text().collect::<String>());
                let href = a.value().attr("href").unwrap_or("");
                let link = if href.starts_with("http") {
                    href.to_string()
                } else {
                    format!("https://www.reddit.com{}", href)
                };
                (title, link)
            } else {
                continue;
            };

            let author = result.select(&scraper::Selector::parse("a.author").unwrap())
                .next()
                .map(|a| format!("u/{}", a.text().collect::<String>()));

            let score_text = result.select(&scraper::Selector::parse("span.search-score").unwrap())
                .next()
                .map(|s| s.text().collect::<String>())
                .unwrap_or_default();
            let score_num: i64 = score_text.replace(',', "").chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0);

            let comments_text = result.select(&scraper::Selector::parse("a.search-comments").unwrap())
                .next()
                .map(|a| a.text().collect::<String>())
                .unwrap_or_default();
            let comments_num: i64 = comments_text.replace(',', "").chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0);

            let pub_date = result.select(&scraper::Selector::parse("time").unwrap())
                .next()
                .and_then(|t| t.value().attr("datetime"))
                .map(|d| d.to_string())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let subreddit = result.select(&scraper::Selector::parse("a.search-subreddit-link").unwrap())
                .next()
                .map(|a| a.text().collect::<String>())
                .map(|s| s.trim_start_matches("r/").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let item_id = parsers::make_item_id(
                &title,
                &link,
                &pub_date,
                &format!("reddit_{}", subreddit),
            );

            posts.push(NewsItem {
                id: item_id,
                title,
                link,
                pub_date,
                source_name: format!("Reddit r/{}", subreddit),
                pool_id: "REDDIT".to_string(),
                content_snippet: format!("Score: {} | Comments: {}", score_num, comments_num),
                author,
                media_url: None,
            });
        }
    }

    Ok(RedditSearchOutput {
        count: posts.len(),
        posts,
        meta: RedditSearchMeta {
            query: input.query,
            subreddits: input.subreddits,
            sort: sort.to_string(),
            time: time.to_string(),
        },
    })
}

pub async fn reddit_feed(input: RedditFeedInput) -> Result<RedditFeedOutput, String> {
    let limit = input.limit.unwrap_or(25).clamp(1, 100) as usize;

    let client = build_reddit_client();
    let mut all_posts = Vec::new();

    // Initial cooldown to avoid triggering Reddit's aggressive rate limiter
    tokio::time::sleep(Duration::from_secs(10)).await;

    for (idx, sub) in input.subreddits.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(Duration::from_secs(15)).await;
        }

        let rss_url = format!("https://www.reddit.com/r/{}/.rss?limit={}", urlencoding(sub), limit);

        let body = match reddit_fetch(&client, &rss_url, "application/atom+xml,application/xml,text/xml").await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Reddit RSS fetch failed for r/{}: {}", sub, e);
                continue;
            }
        };

        if body.is_empty() {
            tracing::warn!("Reddit RSS: empty body for r/{}", sub);
            continue;
        }

        let feed = match feed_parser::parse(body.as_bytes()) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!("Reddit RSS parse failed for r/{}: {} (body_len={}, first_200={})",
                    sub, e, body.len(), &body.chars().take(200).collect::<String>());
                continue;
            }
        };

        let now = chrono::Utc::now().to_rfc3339();

        for entry in feed.entries.iter().take(limit) {
            let title = entry.title.as_ref()
                .map(|t| t.content.clone())
                .unwrap_or_else(|| "Untitled".to_string());

            let link = entry.links.first()
                .map(|l| l.href.clone())
                .or_else(|| {
                    let id = &entry.id;
                    if id.starts_with("http") { Some(id.clone()) } else { None }
                })
                .unwrap_or_default();

            let pub_date = entry.published.or(entry.updated)
                .map(|d: chrono::DateTime<chrono::Utc>| d.to_rfc3339())
                .unwrap_or_else(|| now.clone());

            let content_snippet = entry.summary.as_ref()
                .map(|s| s.content.clone())
                .unwrap_or_default();
            let content_snippet = parsers::strip_html_tags(&content_snippet);
            let content_snippet = content_snippet.chars().take(600).collect::<String>();

            let author = entry.authors.first()
                .map(|a| a.name.clone())
                .filter(|n| !n.is_empty());

            let item_id = parsers::make_item_id(&title, &link, &pub_date, &format!("reddit_rss_{}", sub));

            all_posts.push(NewsItem {
                id: item_id,
                title: title.to_string(),
                link: link.to_string(),
                pub_date: pub_date.to_string(),
                source_name: format!("Reddit r/{}", sub),
                pool_id: "REDDIT".to_string(),
                content_snippet,
                author,
                media_url: None,
            });
        }
    }

    Ok(RedditFeedOutput {
        count: all_posts.len(),
        posts: all_posts,
        subreddits: input.subreddits,
    })
}
