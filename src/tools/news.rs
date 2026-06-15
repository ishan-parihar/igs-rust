use crate::clustering;
use crate::config;
use crate::fusion;
use crate::http::{self as http_mod, HttpClient};
use crate::parsers;
use crate::tools::helpers::*;
use crate::tools::types::*;
use crate::types::*;
use std::sync::Arc;

fn depth_limits(depth: &str) -> (usize, usize) {
    match depth.to_lowercase().as_str() {
        "quick" => (10, 20),
        "deep" => (200, 500),
        _ => (100, 100),
    }
}

/// Fetch normalized news items from configured sources
pub async fn news_fetch(input: NewsFetchInput) -> Result<NewsFetchOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = Arc::new(HttpClient::new(&settings.http, &cache_dir));
    let sf = config::load_sources().await.map_err(|e| format!("Sources: {}", e))?;

    let cache_mode = input.cache_mode.unwrap_or_else(|| "prefer".to_string());
    let depth = input.depth.unwrap_or_else(|| "default".to_string());
    let (max_sources, max_results) = depth_limits(&depth);
    let limit = input.limit.unwrap_or(max_results as i32).clamp(1, 500) as usize;

    let mut sources = sf.sources;
    sources.retain(|s| s.is_active.unwrap_or(true));

    // Filter sources by pool
    if let Some(ref pool_ids) = input.pools {
        if !pool_ids.is_empty() {
            sources.retain(|s| s.pools.iter().any(|p| pool_ids.contains(p)));
        }
    }

    // Filter by country/city/domain
    if let Some(ref countries) = input.countries {
        if !countries.is_empty() {
            sources.retain(|s| {
                s.countries.iter().any(|sc| {
                    countries.iter().any(|c| sc.to_uppercase() == c.to_uppercase())
                })
            });
        }
    }
    if let Some(ref cities) = input.cities {
        if !cities.is_empty() {
            sources.retain(|s| s.cities.iter().any(|c| cities.iter().any(|cc| c.to_lowercase() == cc.to_lowercase())));
        }
    }
    if let Some(ref domains) = input.domains {
        if !domains.is_empty() {
            sources.retain(|s| {
                s.domains.iter().any(|d| domains.iter().any(|dd| d.to_lowercase() == dd.to_lowercase()))
            });
        }
    }

    sources.truncate(max_sources);

    let mut all_items = Vec::new();
    let mut source_groups: Vec<(Vec<NewsItem>, f64)> = Vec::new();
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    let sem = Arc::new(tokio::sync::Semaphore::new(settings.http.concurrency as usize));
    let total = sources.len();

    let mut handles = Vec::new();
    for src in sources.into_iter() {
        let sem = sem.clone();
        let http_ref = http.clone();
        let cm = cache_mode.clone();
        let weight = src.weight.unwrap_or(1.0);
        let src_id = src.id.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");
            match parsers::parse_by_source(&src, &http_ref, &cm, None).await {
                Ok(items) => (src_id, items, weight, true),
                Err(_) => (src_id, vec![], weight, false),
            }
        }));
    }

    for handle in handles {
        match handle.await {
            Ok((_src_id, items, weight, ok)) => {
                if ok {
                    source_groups.push((items.clone(), weight));
                }
                all_items.extend(items);
                if ok { succeeded += 1; } else { failed += 1; }
            }
            Err(_) => { failed += 1; }
        }
    }

    all_items = if source_groups.len() > 1 {
        fusion::weighted_rrf_fusion(source_groups, 60)
    } else {
        all_items.sort_by(|a, b| {
            match (a.freshness_score, b.freshness_score) {
                (Some(fa), Some(fb)) => fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => b.pub_date.cmp(&a.pub_date),
            }
        });
        all_items
    };

    // Time filter
    if input.start.is_some() || input.end.is_some() {
        all_items = parsers::filter_by_time(
            all_items,
            input.start.as_deref(),
            input.end.as_deref(),
        );
    }

    // Keyword filter
    let mut keyword_vec: Vec<String> = Vec::new();
    if let Some(ref kw) = input.keywords {
        if let Some(arr) = kw.as_array() {
            keyword_vec = arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        }
    }
    if !input.discovery_mode.unwrap_or(false) {
        let exclude = input.exclude_keywords.as_ref().cloned().unwrap_or_default();
        all_items = parsers::filter_by_keywords(
            all_items,
            input.keywords.as_ref(),
            &exclude,
            input.match_all.unwrap_or(false),
        );
    }

    // Deduplicate before truncation
    all_items = parsers::batch_similar(all_items, 0.3);
    all_items = parsers::cap_per_author(all_items, 3);

    all_items.truncate(limit);

    let count = all_items.len();

    let meta = NewsFetchMeta {
        sources_queried: total,
        sources_succeeded: succeeded,
        sources_failed: failed,
        total_sources: total,
        pool_ids: input.pools.unwrap_or_default(),
        keywords: keyword_vec,
        count,
    };

    let clusters = if depth == "deep" && all_items.len() >= 5 {
        let article_clusters = clustering::cluster_articles(all_items.clone(), 2);
        Some(article_clusters.into_iter().take(20).map(|c| ClusterInfo {
            representative: c.representative,
            member_count: c.members.len(),
            entities: c.entities,
            source_count: c.source_count,
        }).collect())
    } else {
        None
    };

    Ok(NewsFetchOutput {
        items: all_items,
        count,
        meta,
        clusters,
    })
}

/// Debug helper. Test a single source and return up to 10 items.
pub async fn news_test_source(input: NewsTestInput) -> Result<NewsTestOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    let sf = config::load_sources().await.map_err(|e| format!("Sources: {}", e))?;

    let src = sf.sources.iter().find(|s| s.id == input.id)
        .ok_or_else(|| format!("Source not found: {}", input.id))?;

    let cache_mode = input.cache_mode.as_deref().unwrap_or("bypass");
    let items = parsers::parse_by_source(src, &http, cache_mode, None)
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    let items: Vec<NewsItem> = items.into_iter().take(10).collect();
    let count = items.len();
    Ok(NewsTestOutput { items, count })
}

/// NLP enrichment (offline). Adds basic topics, sentiment, and summary to items.
pub async fn news_enrich(input: NewsEnrichInput) -> Result<NewsEnrichOutput, String> {
    let extract = input.extract.unwrap_or_else(|| vec![
        "topics".into(), "entities".into(), "sentiment".into(), "summary".into(),
    ]);
    let want: std::collections::HashSet<String> = extract.into_iter().collect();

    let mut out = Vec::new();
    for item in &input.items {
        let text = format!("{} {}", item.title, item.content_snippet.as_deref().unwrap_or(""));

        let mut enriched = serde_json::json!({
            "id": item.id,
            "title": item.title,
            "link": item.link,
            "pub_date": item.pub_date,
            "source_name": item.source_name,
            "pool_id": item.pool_id,
            "content_snippet": item.content_snippet,
            "date_confidence": item.date_confidence,
            "freshness_score": item.freshness_score,
        });

        if want.contains("topics") {
            let topics = extract_topics(&text, 8);
            enriched["topics"] = serde_json::json!(topics);
        }

        if want.contains("entities") {
            let entities = extract_basic_entities(&text);
            enriched["entities"] = serde_json::json!(entities);
        }

        if want.contains("sentiment") {
            let sentiment = basic_sentiment(&text);
            enriched["sentiment"] = serde_json::json!(sentiment);
        }

        if want.contains("summary") {
            let summary = item.content_snippet.as_deref()
                .and_then(|s| s.split(['.', '!', '?'])
                    .find(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string()))
                .unwrap_or_else(|| item.title.clone());
            enriched["summary"] = serde_json::json!(summary);
        }

        if want.contains("diversity") {
            let title_words: Vec<String> = item.title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .map(|s| s.to_string())
                .collect();
            let same_source_count = input.items.iter().filter(|other| {
                other.id != item.id && other.source_name != item.source_name
                    && title_words.iter().any(|tw| {
                        other.title.to_lowercase().split_whitespace().any(|ow| ow == tw)
                    })
            }).count();
            let total_sources: usize = input.items.iter()
                .map(|i| &i.source_name)
                .collect::<std::collections::HashSet<_>>()
                .len();
            let diversity = if total_sources <= 1 { 0.0 }
                else { (same_source_count as f64 / total_sources.max(1) as f64).min(1.0) };
            enriched["source_diversity"] = serde_json::json!(diversity);
        }

        out.push(enriched);
    }

    Ok(NewsEnrichOutput {
        items: out,
        meta: serde_json::json!({
            "items_enriched": input.items.len(),
            "note": "Basic offline NLP enrichment (no external API calls)"
        }),
    })
}
