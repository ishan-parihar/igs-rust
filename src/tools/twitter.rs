use agent_twitter_client::scraper::Scraper;
use agent_twitter_client::search::SearchMode;

use crate::config;
use crate::tools::types::*;

fn convert_tweet(tweet: agent_twitter_client::models::tweets::Tweet) -> TwitterTweet {
    let id = tweet.id.unwrap_or_default();
    let username = tweet.username.clone().unwrap_or_default();
    TwitterTweet {
        id: id.clone(),
        text: tweet.text.unwrap_or_default(),
        author: tweet.name.unwrap_or_default(),
        username: username.clone(),
        created_at: tweet.created_at.unwrap_or_default(),
        url: tweet
            .permanent_url
            .unwrap_or_else(|| format!("https://x.com/{}/status/{}", username, id)),
        likes: tweet.likes,
        retweets: tweet.retweets,
        replies: tweet.replies,
        views: tweet.ext_views.or(tweet.views),
        is_retweet: tweet.is_retweet.unwrap_or(false),
        is_reply: tweet.is_reply.unwrap_or(false),
        hashtags: tweet.hashtags,
        urls: tweet.urls,
    }
}

fn parse_search_mode(mode: &str) -> SearchMode {
    match mode.to_lowercase().as_str() {
        "top" => SearchMode::Top,
        "photos" => SearchMode::Photos,
        "videos" => SearchMode::Videos,
        "users" => SearchMode::Users,
        _ => SearchMode::Latest,
    }
}

async fn get_scraper() -> Result<Scraper, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let twitter = settings.twitter.as_ref().ok_or("Twitter not configured")?;
    if !twitter.enabled {
        return Err(
            "Twitter integration is disabled. Set twitter.enabled=true in settings.yml".into(),
        );
    }
    let cookie = twitter
        .cookie
        .as_deref()
        .ok_or("Twitter cookie not configured. Set twitter.cookie in settings.yml")?;
    let mut scraper = Scraper::new()
        .await
        .map_err(|e| format!("Failed to create scraper: {}", e))?;
    scraper
        .set_from_cookie_string(cookie)
        .await
        .map_err(|e| format!("Failed to authenticate: {}", e))?;
    Ok(scraper)
}

pub async fn twitter_search(input: TwitterSearchInput) -> Result<TwitterSearchOutput, String> {
    let scraper = get_scraper().await?;
    let limit = input.limit.unwrap_or(10).clamp(1, 100) as i32;
    let mode = input
        .search_mode
        .as_deref()
        .map(parse_search_mode)
        .unwrap_or(SearchMode::Latest);

    let response = scraper
        .search_tweets(&input.query, limit, mode, None)
        .await
        .map_err(|e| format!("Twitter search failed: {}", e))?;

    let tweets: Vec<TwitterTweet> = response.tweets.into_iter().map(convert_tweet).collect();
    let count = tweets.len();

    Ok(TwitterSearchOutput {
        tweets,
        count,
        query: input.query,
    })
}

pub async fn twitter_read(input: TwitterReadInput) -> Result<TwitterReadOutput, String> {
    let scraper = get_scraper().await?;

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

    let tweet = scraper
        .get_tweet(tweet_id)
        .await
        .map_err(|e| format!("Failed to fetch tweet: {}", e))?;

    Ok(TwitterReadOutput {
        tweet: convert_tweet(tweet),
    })
}
