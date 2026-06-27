// ─── Twitter/X Integration ──────────────────────────────────
// Cookie-based GraphQL API via reqwest with native-tls.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};

use crate::config;
use crate::tools::types::*;

// ── Constants ───────────────────────────────────────────────

const BEARER_TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA";

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

static QUERY_IDS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("SearchTimeline", "VhUd6vHVmLBcw0uX-6jMLA");
    m.insert("TweetResultByRestId", "7xflPyRiUxGVbJd4uWmbfg");
    m
});

static GRAPHQL_FEATURES: LazyLock<serde_json::Value> = LazyLock::new(|| {
    serde_json::json!({
        "responsive_web_graphql_exclude_directive_enabled": true,
        "verified_phone_label_enabled": false,
        "creator_subscriptions_tweet_preview_api_enabled": true,
        "responsive_web_graphql_timeline_navigation_enabled": true,
        "responsive_web_graphql_skip_user_profile_image_extensions_enabled": false,
        "c9s_tweet_anatomy_moderator_badge_enabled": true,
        "tweetypie_unmention_optimization_enabled": true,
        "responsive_web_edit_tweet_api_enabled": true,
        "graphql_is_translatable_rweb_tweet_is_translatable_enabled": true,
        "view_counts_everywhere_api_enabled": true,
        "longform_notetweets_consumption_enabled": true,
        "responsive_web_twitter_article_tweet_consumption_enabled": true,
        "tweet_awards_web_tipping_enabled": false,
        "longform_notetweets_rich_text_read_enabled": true,
        "longform_notetweets_inline_media_enabled": true,
        "rweb_video_timestamps_enabled": true,
        "responsive_web_media_download_video_enabled": true,
        "freedom_of_speech_not_reach_fetch_enabled": true,
        "standardized_nudges_misinfo": true,
        "responsive_web_enhance_cards_enabled": false
    })
});

// ── Cookie Parsing ─────────────────────────────────────────

fn parse_cookie_string(cookie_str: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in cookie_str.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

fn extract_ct0(cookies: &HashMap<String, String>) -> Option<String> {
    cookies.get("ct0").cloned()
}

fn ensure_ct0_matches(cookie_str: &str, ct0: &str) -> String {
    let parts: Vec<String> = cookie_str
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.starts_with("ct0=") && !s.is_empty())
        .collect();
    let mut result = parts.join("; ");
    if !result.is_empty() {
        result.push_str("; ");
    }
    result.push_str(&format!("ct0={ct0}"));
    result
}

// ── HTTP Client ────────────────────────────────────────────

fn build_client(cookie_str: &str) -> Result<reqwest::Client, String> {
    let cookies = parse_cookie_string(cookie_str);
    let ct0 = extract_ct0(&cookies).unwrap_or_default();

    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
    headers.insert("accept", HeaderValue::from_static("*/*"));
    headers.insert("origin", HeaderValue::from_static("https://x.com"));
    headers.insert("referer", HeaderValue::from_static("https://x.com/"));
    headers.insert("x-twitter-active-user", HeaderValue::from_static("yes"));
    headers.insert("x-twitter-auth-type", HeaderValue::from_static("OAuth2Session"));
    headers.insert("x-twitter-client-language", HeaderValue::from_static("en"));
    headers.insert("authorization", HeaderValue::from_str(&format!("Bearer {BEARER_TOKEN}")).unwrap());
    headers.insert("x-csrf-token", HeaderValue::from_str(&ct0).unwrap());
    headers.insert("cookie", HeaderValue::from_str(cookie_str).unwrap());

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

fn graphql_url(query_id: &str, operation: &str, variables: &serde_json::Value) -> String {
    let features = GRAPHQL_FEATURES.to_string();
    let vars = variables.to_string();
    let params = format!("variables={vars}&features={features}");
    let encoded = urlencoding::encode(&params);
    format!("https://x.com/i/api/graphql/{query_id}/{operation}?{encoded}")
}

// ── GraphQL Request ────────────────────────────────────────

async fn graphql_post(
    client: &reqwest::Client,
    query_id: &str,
    operation: &str,
    variables: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let url = format!("https://x.com/i/api/graphql/{query_id}/{operation}");
    let body = serde_json::json!({
        "variables": variables,
        "queryId": query_id,
        "features": *GRAPHQL_FEATURES,
    });

    let resp = client
        .post(&url)
        .header("Priority", "u=1, i")
        .header("Referer", "https://x.com/compose/post")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Body error: {e}"))?;

    if !status.is_success() {
        return Err(format!("HTTP {status}: {body}"));
    }

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {e}"))?;
    Ok(json)
}

// ── Tweet Extraction ───────────────────────────────────────

fn extract_tweet_from_result(result: &serde_json::Value) -> Option<TwitterTweet> {
    let legacy = result.get("legacy")?;
    let id = legacy.get("id_str")?.as_str()?.to_string();
    let full_text = legacy.get("full_text")?.as_str()?.to_string();
    let created_at = legacy.get("created_at")?.as_str()?.to_string();

    let user = result
        .pointer("/core/user_results/result/legacy")?;
    let name = user.get("name")?.as_str()?.to_string();
    let screen_name = user.get("screen_name")?.as_str()?.to_string();

    let likes = legacy.get("favorite_count").and_then(|v| v.as_i64()).map(|v| v as i32);
    let retweets = legacy.get("retweet_count").and_then(|v| v.as_i64()).map(|v| v as i32);
    let replies = legacy.get("reply_count").and_then(|v| v.as_i64()).map(|v| v as i32);

    let views = legacy
        .get("views")
        .and_then(|v| v.get("count"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .map(|v| v as i32);

    let hashtags: Vec<String> = legacy
        .get("entities")
        .and_then(|e| e.get("hashtags"))
        .and_then(|h| h.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|h| h.get("text").and_then(|t| t.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let urls: Vec<String> = legacy
        .get("entities")
        .and_then(|e| e.get("urls"))
        .and_then(|u| u.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|u| u.get("expanded_url").and_then(|t| t.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let is_retweet = full_text.starts_with("RT @");
    let is_reply = legacy.get("in_reply_to_status_id_str")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    Some(TwitterTweet {
        id: id.clone(),
        text: full_text,
        author: name,
        username: screen_name.clone(),
        created_at,
        url: format!("https://x.com/{screen_name}/status/{id}"),
        likes,
        retweets,
        replies,
        views,
        is_retweet,
        is_reply,
        hashtags,
        urls,
    })
}

fn extract_tweets_from_timeline(data: &serde_json::Value) -> Vec<TwitterTweet> {
    let mut tweets = Vec::new();

    // Navigate timeline instructions
    let instructions = data
        .pointer("/data/search_by_raw_query/search_timeline/timeline/instructions")
        .or_else(|| data.pointer("/data/user/result/timeline_v2/timeline/instructions"))
        .and_then(|i| i.as_array());

    if let Some(instructions) = instructions {
        for instruction in instructions {
            let entries = instruction
                .get("entries")
                .and_then(|e| e.as_array());

            if let Some(entries) = entries {
                for entry in entries {
                    let content = entry.get("content").or(Some(entry));
                    if let Some(items) = content
                        .and_then(|c| c.get("itemContent"))
                        .and_then(|ic| ic.get("items"))
                        .and_then(|i| i.as_array())
                    {
                        for item in items {
                            if let Some(tweet) = item
                                .get("item")
                                .and_then(|i| i.get("tweet_results"))
                                .and_then(|tr| tr.get("result"))
                                .and_then(extract_tweet_from_result)
                            {
                                tweets.push(tweet);
                            }
                        }
                    }
                    // Single item format
                    if let Some(tweet) = content
                        .and_then(|c| c.get("itemContent"))
                        .and_then(|ic| ic.get("tweet_results"))
                        .and_then(|tr| tr.get("result"))
                        .and_then(extract_tweet_from_result)
                    {
                        tweets.push(tweet);
                    }
                    // Module items format
                    if let Some(items) = content
                        .and_then(|c| c.get("items"))
                        .and_then(|i| i.as_array())
                    {
                        for item in items {
                            if let Some(tweet) = item
                                .get("item")
                                .and_then(|i| i.get("tweet_results"))
                                .and_then(|tr| tr.get("result"))
                                .and_then(extract_tweet_from_result)
                            {
                                tweets.push(tweet);
                            }
                        }
                    }
                }
            }
        }
    }

    tweets
}

// ── Public API ─────────────────────────────────────────────

pub async fn twitter_search(input: TwitterSearchInput) -> Result<TwitterSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {e}"))?;
    let twitter = settings.twitter.as_ref().ok_or("Twitter not configured")?;
    if !twitter.enabled {
        return Err("Twitter integration is disabled. Set twitter.enabled=true in settings.yml".into());
    }
    let cookie = twitter
        .cookie
        .as_deref()
        .ok_or("Twitter cookie not configured. Set twitter.cookie in settings.yml")?;

    let client = build_client(cookie)?;
    let limit = input.limit.unwrap_or(10).clamp(1, 100);

    let product = input
        .search_mode
        .as_deref()
        .map(|m| match m.to_lowercase().as_str() {
            "top" => "Top",
            "photos" => "Photos",
            "videos" => "Videos",
            "users" => "People",
            _ => "Latest",
        })
        .unwrap_or("Latest");

    let variables = serde_json::json!({
        "rawQuery": input.query,
        "count": limit,
        "querySource": "typed_query",
        "product": product,
    });

    let qid = QUERY_IDS.get("SearchTimeline").unwrap();
    let json = graphql_post(&client, qid, "SearchTimeline", &variables).await?;

    tracing::debug!("Twitter search response: {}", serde_json::to_string_pretty(&json).unwrap_or_default());

    let tweets = extract_tweets_from_timeline(&json);
    let count = tweets.len();

    Ok(TwitterSearchOutput {
        tweets,
        count,
        query: input.query,
    })
}

pub async fn twitter_read(input: TwitterReadInput) -> Result<TwitterReadOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {e}"))?;
    let twitter = settings.twitter.as_ref().ok_or("Twitter not configured")?;
    if !twitter.enabled {
        return Err("Twitter integration is disabled. Set twitter.enabled=true in settings.yml".into());
    }
    let cookie = twitter
        .cookie
        .as_deref()
        .ok_or("Twitter cookie not configured. Set twitter.cookie in settings.yml")?;

    let client = build_client(cookie)?;

    let tweet_id = if input.url.contains("/status/") {
        input
            .url
            .split("/status/")
            .nth(1)
            .and_then(|s| s.split('?').next())
            .unwrap_or(&input.url)
    } else {
        input.url.trim()
    };

    let variables = serde_json::json!({
        "tweetId": tweet_id,
        "withCommunity": false,
        "includePromotedContent": false,
        "withQuickPromoteEligibilityTweetFields": false,
        "withBirdwatchNotes": false,
        "withVoice": false,
    });

    let qid = QUERY_IDS.get("TweetResultByRestId").unwrap();
    let json = graphql_post(&client, qid, "TweetResultByRestId", &variables).await?;

    let tweet = json
        .pointer("/data/tweet_result/result")
        .and_then(extract_tweet_from_result)
        .ok_or_else(|| format!("Tweet not found: {tweet_id}"))?;

    Ok(TwitterReadOutput { tweet })
}
