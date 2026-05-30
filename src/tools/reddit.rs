use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::parsers;
use crate::tools::helpers::urlencoding;
use crate::tools::types::*;
use crate::types::*;

/// Search Reddit posts. Uses reddit.com JSON API.
pub async fn reddit_search(input: RedditSearchInput) -> Result<RedditSearchOutput, String> {
    let sort = input.sort.as_deref().unwrap_or("relevance");
    let time = input.time.as_deref().unwrap_or("all");
    let limit = input.limit.unwrap_or(25).min(100).max(1);

    let query_enc = urlencoding(&input.query);
    let subreddit_filter = input.subreddits.as_ref()
        .map(|sr| sr.join("+"))
        .unwrap_or_default();

    let api_url = if subreddit_filter.is_empty() {
        format!("https://www.reddit.com/search.json?q={}&sort={}&t={}&limit={}",
            query_enc, sort, time, limit)
    } else {
        format!("https://www.reddit.com/r/{}/search.json?q={}&sort={}&t={}&limit={}",
            subreddit_filter, query_enc, sort, time, limit)
    };

    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    match http.fetch(&api_url, None, "bypass").await {
        Ok(outcome) => {
            let body = match outcome {
                http_mod::FetchOutcome::Cached(entry) => {
                    let posts: Vec<NewsItem> = entry.items;
                    return Ok(RedditSearchOutput {
                        count: posts.len(),
                        posts,
                        meta: RedditSearchMeta {
                            query: input.query,
                            subreddits: input.subreddits,
                            sort: sort.to_string(),
                            time: time.to_string(),
                        },
                    });
                }
                http_mod::FetchOutcome::Response(resp, _, _) => resp.body_text,
            };

            let json: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| format!("Failed to parse Reddit response: {}", e))?;

            let posts: Vec<NewsItem> = json["data"]["children"]
                .as_array()
                .map(|children| {
                    children.iter().filter_map(|child| {
                        let data = &child["data"];
                        let title = data["title"].as_str().unwrap_or("Untitled");
                        let permalink = data["permalink"].as_str().unwrap_or("");
                        let url = format!("https://www.reddit.com{}", permalink);
                        let subreddit = data["subreddit"].as_str().unwrap_or("unknown");
                        let author = data["author"].as_str();
                        let score = data["score"].as_i64().unwrap_or(0);
                        let num_comments = data["num_comments"].as_i64().unwrap_or(0);
                        let created_utc = data["created_utc"].as_f64().unwrap_or(0.0);
                        let selftext = data["selftext"].as_str().unwrap_or("");
                        let created = chrono::DateTime::from_timestamp(created_utc as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                        let item_id = parsers::make_item_id(
                            title,
                            &url,
                            &created,
                            &format!("reddit_{}", subreddit),
                        );

                        Some(NewsItem {
                            id: item_id,
                            title: title.to_string(),
                            link: url,
                            pub_date: created,
                            source_name: format!("Reddit r/{}", subreddit),
                            pool_id: "REDDIT".to_string(),
                            content_snippet: format!("Score: {} | Comments: {} | {}", score, num_comments,
                                selftext.chars().take(300).collect::<String>()),
                            author: author.map(|a| a.to_string()),
                            media_url: None,
                        })
                    }).collect()
                })
                .unwrap_or_default();

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
        Err(e) => Err(format!("Reddit search failed: {}", e)),
    }
}
