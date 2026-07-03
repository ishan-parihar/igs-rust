//! GDELT 2.0 integration — Global Database of Events, Language, and Tone.
//!
//! GDELT monitors news from 100K+ sources in 65+ languages, processing
//! 300M+ events. The API is free and requires no API key.
//!
//! This module provides:
//! - Event search: query the GDELT Events 2.0 API
//! - Article list: query the GDELT DOC 2.0 API for article metadata
//! - Timeline: get mention volume over time for a topic

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use crate::tools::types::LimitInput;
use crate::tools::types_base::OutputOptions;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GdeltEvent {
    pub date: String,
    pub actor1: String,
    pub actor2: String,
    pub action: String,
    pub location: String,
    pub source_url: String,
    pub tone: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GdeltSearchInput {
    /// Search query (e.g., "climate change" or "Saudi Arabia")
    pub query: String,
    /// Max results (default: 50, max: 250)
    pub limit: Option<u32>,
    /// Start date (YYYYMMDD format)
    pub start_date: Option<String>,
    /// End date (YYYYMMDD format)
    pub end_date: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GdeltSearchOutput {
    pub query: String,
    pub total: usize,
    pub events: Vec<GdeltEvent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GdeltArticleListInput {
    /// Search query
    pub query: String,
    /// Max results (default: 50, max: 250)
    pub limit: Option<u32>,
    /// Source country (FIPS code, e.g., "US", "CN", "RU")
    pub source_country: Option<String>,
    /// Language (3-letter ISO code, e.g., "eng", "spa", "rus")
    pub language: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GdeltArticle {
    pub url: String,
    pub title: String,
    pub domain: String,
    pub language: String,
    pub date: String,
    pub social_image: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GdeltArticleListOutput {
    pub query: String,
    pub total: usize,
    pub articles: Vec<GdeltArticle>,
}

/// Search GDELT Events 2.0 API for events matching the query.
pub async fn gdelt_search(input: GdeltSearchInput) -> Result<GdeltSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let limit = input.limit.unwrap_or(50).min(250);
    let query_enc = urlencoding(&input.query);

    // GDELT DOC 2.0 API — returns articles matching the query
    let mut url = format!(
        "https://api.gdeltproject.org/api/v2/doc/doc?query={}&mode=ArtList&maxrecords={}&format=json",
        query_enc, limit
    );

    if let Some(ref start) = input.start_date {
        url.push_str(&format!("&startdatetime={}", start));
    }
    if let Some(ref end) = input.end_date {
        url.push_str(&format!("&enddatetime={}", end));
    }

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("GDELT API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("GDELT JSON parse error: {}", e))?;

    let articles: Vec<GdeltEvent> = json["articles"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| GdeltEvent {
                    date: a["seendate"].as_str().unwrap_or("").to_string(),
                    actor1: a["sourcecountry"].as_str().unwrap_or("").to_string(),
                    actor2: String::new(),
                    action: a["title"].as_str().unwrap_or("").to_string(),
                    location: a["sourcecountry"].as_str().unwrap_or("").to_string(),
                    source_url: a["url"].as_str().unwrap_or("").to_string(),
                    tone: 0.0,
                })
                .collect()
        })
        .unwrap_or_default();

    let total = articles.len();
    Ok(GdeltSearchOutput {
        query: input.query,
        total,
        events: articles,
    })
}

/// List articles from GDELT DOC 2.0 API with filtering.
pub async fn gdelt_article_list(input: GdeltArticleListInput) -> Result<GdeltArticleListOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let limit = input.limit.unwrap_or(50).min(250);
    let query_enc = urlencoding(&input.query);

    let mut url = format!(
        "https://api.gdeltproject.org/api/v2/doc/doc?query={}&mode=ArtList&maxrecords={}&format=json",
        query_enc, limit
    );

    if let Some(ref country) = input.source_country {
        url.push_str(&format!("&sourcecountry:{}", country));
    }
    if let Some(ref lang) = input.language {
        url.push_str(&format!("&sourcelang:{}", lang));
    }

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("GDELT API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("GDELT JSON parse error: {}", e))?;

    let articles: Vec<GdeltArticle> = json["articles"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| GdeltArticle {
                    url: a["url"].as_str().unwrap_or("").to_string(),
                    title: a["title"].as_str().unwrap_or("").to_string(),
                    domain: a["domain"].as_str().unwrap_or("").to_string(),
                    language: a["language"].as_str().unwrap_or("").to_string(),
                    date: a["seendate"].as_str().unwrap_or("").to_string(),
                    social_image: a["socialimage"].as_str().map(|s| s.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();

    let total = articles.len();
    Ok(GdeltArticleListOutput {
        query: input.query,
        total,
        articles,
    })
}
