use crate::config;
use crate::parsers;
use crate::tools::helpers::urlencoding;
use crate::tools::types::*;
use crate::types::*;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

const REDDIT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Brave/Chrome/145.0.0.0 Safari/537.36";

// ─── Reddit API Response Types ─────────────────────────────────

#[derive(Debug, Deserialize)]
struct RedditListingResponse {
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
struct RedditListingData {
    children: Vec<RedditChild>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedditChild {
    data: RedditPost,
}

#[derive(Debug, Deserialize)]
struct RedditPost {
    title: String,
    permalink: String,
    author: String,
    subreddit: String,
    score: i64,
    num_comments: i64,
    created_utc: f64,
    selftext: Option<String>,
    url: Option<String>,
    thumbnail: Option<String>,
    is_self: bool,
}

// ─── Cookie-based Authentication ──────────────────────────────

/// Load Reddit cookie from settings
async fn load_reddit_cookie() -> Result<String, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings load failed: {}", e))?;

    settings
        .reddit
        .and_then(|r| r.cookie)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| {
            "Reddit cookie not configured. Add reddit.cookie to settings.yml".to_string()
        })
}

/// Build a dedicated reqwest Client for Reddit with browser-like headers.
fn build_reddit_client(cookie: &str) -> Client {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(REDDIT_USER_AGENT));
    headers.insert(
        COOKIE,
        HeaderValue::from_str(cookie).expect("Invalid cookie header"),
    );
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );

    Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("Failed to build Reddit HTTP client")
}

/// GET request via www.reddit.com with cookie auth
async fn reddit_get(client: &Client, url: &str) -> Result<String, String> {
    let max_retries = 3;
    let mut last_err = String::new();

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_secs(2u64.pow(attempt as u32));
            tracing::info!(
                "Reddit: retry {} after {}s for {}",
                attempt,
                delay.as_secs(),
                url
            );
            tokio::time::sleep(delay).await;
        }

        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();

                if status == 429 {
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(10);

                    tracing::warn!(
                        "Reddit: 429 rate-limited for {}, Retry-After: {}s",
                        url,
                        retry_after
                    );
                    tokio::time::sleep(Duration::from_secs(retry_after)).await;
                    last_err = format!("429 rate-limited (Retry-After: {}s)", retry_after);
                    continue;
                }

                if status == 401 || status == 403 {
                    return Err(format!(
                        "Reddit auth failed (HTTP {}). Check your cookie in settings.yml.",
                        status
                    ));
                }

                if status >= 400 {
                    return Err(format!("HTTP {} from {}", status, url));
                }

                return resp.text().await.map_err(|e| format!("Read error: {}", e));
            }
            Err(e) => {
                last_err = e.to_string();
                tracing::warn!(
                    "Reddit: request error for {} (attempt {}): {}",
                    url,
                    attempt,
                    e
                );
            }
        }
    }

    Err(format!(
        "Failed after {} retries: {}",
        max_retries + 1,
        last_err
    ))
}

// ─── Reddit Search (JSON API) ─────────────────────────────────

pub async fn reddit_search(input: RedditSearchInput) -> Result<RedditSearchOutput, String> {
    if input.query.trim().is_empty() {
        return Err("Query cannot be empty".to_string());
    }

    let sort = input.sort.as_deref().unwrap_or("relevance");
    let time = input.time.as_deref().unwrap_or("all");
    let limit = input.limit.unwrap_or(25).clamp(1, 100);

    let cookie = load_reddit_cookie().await?;
    let client = build_reddit_client(&cookie);
    let mut posts = Vec::new();

    let subreddits = input.subreddits.clone().unwrap_or_default();
    let search_urls = if subreddits.is_empty() {
        vec![format!(
            "https://www.reddit.com/search.json?q={}&sort={}&t={}&limit={}&raw_json=1",
            urlencoding(&input.query),
            sort,
            time,
            limit
        )]
    } else {
        subreddits.iter().map(|sr| {
            format!(
                "https://www.reddit.com/r/{}/search.json?q={}&restrict_sr=on&sort={}&t={}&limit={}&raw_json=1",
                urlencoding(sr), urlencoding(&input.query), sort, time, limit
            )
        }).collect()
    };

    for (idx, url) in search_urls.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        let body = match reddit_get(&client, url).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Reddit search failed for {}: {}", url, e);
                continue;
            }
        };

        let listing: RedditListingResponse = match serde_json::from_str(&body) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Reddit JSON parse failed for {}: {}", url, e);
                continue;
            }
        };

        for child in listing.data.children.into_iter().take(limit as usize) {
            let post = child.data;
            let link = format!("https://www.reddit.com{}", post.permalink);
            let pub_date = chrono::DateTime::from_timestamp(post.created_utc as i64, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let item_id = parsers::make_item_id(
                &post.title,
                &link,
                &pub_date,
                &format!("reddit_{}", post.subreddit),
            );

            posts.push(NewsItem {
                id: item_id,
                title: post.title,
                link,
                pub_date,
                source_name: format!("Reddit r/{}", post.subreddit),
                pool_id: "REDDIT".to_string(),
                content_snippet: format!("Score: {} | Comments: {}", post.score, post.num_comments),
                author: Some(format!("u/{}", post.author)),
                media_url: None,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(parsers::calculate_freshness(
                    &chrono::DateTime::from_timestamp(post.created_utc as i64, 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_default(),
                )),
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

// ─── Reddit Feed (JSON API) ───────────────────────────────────

pub async fn reddit_feed(input: RedditFeedInput) -> Result<RedditFeedOutput, String> {
    let limit = input.limit.unwrap_or(25).clamp(1, 100) as usize;

    let cookie = load_reddit_cookie().await?;
    let client = build_reddit_client(&cookie);
    let mut all_posts = Vec::new();

    for (idx, sub) in input.subreddits.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        let url = format!(
            "https://www.reddit.com/r/{}/hot.json?limit={}&raw_json=1",
            urlencoding(sub),
            limit
        );

        let body = match reddit_get(&client, &url).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("Reddit feed failed for r/{}: {}", sub, e);
                continue;
            }
        };

        let listing: RedditListingResponse = match serde_json::from_str(&body) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Reddit JSON parse failed for r/{}: {}", sub, e);
                continue;
            }
        };

        for child in listing.data.children.into_iter().take(limit) {
            let post = child.data;
            let link = format!("https://www.reddit.com{}", post.permalink);
            let pub_date = chrono::DateTime::from_timestamp(post.created_utc as i64, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let content_snippet = if post.is_self {
                post.selftext.unwrap_or_default()
            } else {
                post.url.unwrap_or_default()
            };
            let content_snippet = content_snippet.chars().take(600).collect::<String>();

            let item_id =
                parsers::make_item_id(&post.title, &link, &pub_date, &format!("reddit_{}", sub));

            all_posts.push(NewsItem {
                id: item_id,
                title: post.title,
                link,
                pub_date,
                source_name: format!("Reddit r/{}", sub),
                pool_id: "REDDIT".to_string(),
                content_snippet,
                author: Some(format!("u/{}", post.author)),
                media_url: None,
                date_confidence: Some("high".to_string()),
                freshness_score: Some(parsers::calculate_freshness(
                    &chrono::DateTime::from_timestamp(post.created_utc as i64, 0)
                        .map(|d| d.to_rfc3339())
                        .unwrap_or_default(),
                )),
            });
        }
    }

    Ok(RedditFeedOutput {
        count: all_posts.len(),
        posts: all_posts,
        subreddits: input.subreddits,
    })
}
