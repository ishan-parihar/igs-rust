use crate::clustering;
use crate::config;
use crate::fusion;
use crate::http::{self as http_mod, HttpClient};
use crate::parsers;
use crate::server::InsightStorage;
use crate::tools::helpers::*;
use crate::tools::types::*;
use crate::tools::types_base::{KeywordFilter, OutputOptions};
use crate::types::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Return (max_sources, max_results) for a given depth string.
/// - "quick"   → (10, 20)   — fast, few sources, few results
/// - "deep"    → (200, 500) — full pipeline, many sources, many results
/// - "default" (or any other value, including the unwrap_or when depth is None)
///   → (100, 100) — balanced default
fn depth_limits(depth: &str) -> (usize, usize) {
    match depth.to_lowercase().as_str() {
        "quick" => (10, 20),
        "deep" => (200, 500),
        _ => (100, 100),
    }
}

/// Serialize a NewsItem's core fields into a JSON object for the EnrichedItem.item field.
/// Shared between `fetch_news_intelligent` (skip_enrich path) and `news_enrich` to avoid
/// duplicating the 9-field `serde_json::json!({...})` block.
fn news_item_to_json(item: &NewsItem) -> serde_json::Value {
    serde_json::json!({
        "id": item.id,
        "title": item.title,
        "link": item.link,
        "pub_date": item.pub_date,
        "source_name": item.source_name,
        "pool_id": item.pool_id,
        "content_snippet": item.content_snippet,
        "date_confidence": item.date_confidence,
        "freshness_score": item.freshness_score,
    })
}

/// Fetch normalized news items from configured sources
pub async fn news_fetch(input: NewsFetchInput) -> Result<NewsFetchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = Arc::new(HttpClient::new(&settings.http, &cache_dir));
    let sf = config::load_sources()
        .await
        .map_err(|e| format!("Sources: {}", e))?;

    let cache_mode = input
        .filters
        .cache_mode
        .unwrap_or_else(|| "prefer".to_string());
    let depth = input
        .depth_opts
        .depth
        .unwrap_or_else(|| "default".to_string());
    let (max_sources, max_results) = depth_limits(&depth);
    let limit = input
        .filters
        .limit
        .unwrap_or(max_results as i32)
        .clamp(1, 500) as usize;

    let mut sources = sf.sources;
    sources.retain(|s| s.is_active.unwrap_or(true));

    // Filter sources by pool
    if let Some(ref pool_ids) = input.filters.pools {
        if !pool_ids.is_empty() {
            sources.retain(|s| s.pools.iter().any(|p| pool_ids.contains(p)));
        }
    }

    // Filter by country/city/domain
    if let Some(ref countries) = input.filters.countries {
        if !countries.is_empty() {
            sources.retain(|s| {
                s.countries.iter().any(|sc| {
                    countries
                        .iter()
                        .any(|c| sc.to_uppercase() == c.to_uppercase())
                })
            });
        }
    }
    if let Some(ref cities) = input.filters.cities {
        if !cities.is_empty() {
            sources.retain(|s| {
                s.cities.iter().any(|c| {
                    cities
                        .iter()
                        .any(|cc| c.to_lowercase() == cc.to_lowercase())
                })
            });
        }
    }
    if let Some(ref domains) = input.filters.domains {
        if !domains.is_empty() {
            sources.retain(|s| {
                s.domains.iter().any(|d| {
                    domains
                        .iter()
                        .any(|dd| d.to_lowercase() == dd.to_lowercase())
                })
            });
        }
    }

    sources.truncate(max_sources);

    // Partition out reddit/twitter-platform sources — these are fetched via
    // dedicated `reddit.search`/`reddit.feed` and `twitter.search`/`twitter.read`
    // tools, not via `news.fetch`. Previously they were sent to `parse_by_source`
    // which silently returned `vec![]` for them, but they were counted as
    // "succeeded" in the meta, inflating the success metric.
    let short_circuited = sources
        .iter()
        .filter(|s| matches!(s.platform.as_deref(), Some("reddit") | Some("twitter")))
        .count();
    sources.retain(|s| !matches!(s.platform.as_deref(), Some("reddit") | Some("twitter")));

    let mut all_items = Vec::new();
    let mut source_groups: Vec<(Vec<NewsItem>, f64)> = Vec::new();
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    let sem = Arc::new(tokio::sync::Semaphore::new(
        settings.http.concurrency as usize,
    ));
    let total = sources.len() + short_circuited;

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
                if ok {
                    succeeded += 1;
                } else {
                    failed += 1;
                }
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    all_items = if source_groups.len() > 1 {
        fusion::weighted_rrf_fusion(source_groups, 60)
    } else {
        all_items.sort_by(|a, b| match (a.freshness_score, b.freshness_score) {
            (Some(fa), Some(fb)) => fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => b.pub_date.cmp(&a.pub_date),
        });
        all_items
    };

    // Time filter
    if input.filters.start.is_some() || input.filters.end.is_some() {
        all_items = parsers::filter_by_time(
            all_items,
            input.filters.start.as_deref(),
            input.filters.end.as_deref(),
        );
    }

    // Keyword filter
    let mut keyword_vec: Vec<String> = Vec::new();
    if let Some(ref kw) = input.filters.keywords {
        match kw {
            KeywordFilter::Single(s) => keyword_vec.push(s.clone()),
            KeywordFilter::Multiple(arr) => keyword_vec.extend(arr.iter().cloned()),
            KeywordFilter::Nested(nested) => {
                for cluster in nested.iter() {
                    keyword_vec.extend(cluster.iter().cloned());
                }
            }
        }
    }
    if !input.discovery_mode.unwrap_or(false) {
        let exclude = input
            .filters
            .exclude_keywords
            .as_ref()
            .cloned()
            .unwrap_or_default();
        all_items = parsers::filter_by_keywords(
            all_items,
            input.filters.keywords.as_ref(),
            &exclude,
            input.filters.match_all.unwrap_or(false),
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
        sources_short_circuited: short_circuited,
        total_sources: total,
        pool_ids: input.filters.pools.unwrap_or_default(),
        keywords: keyword_vec,
        count,
    };

    let clusters = if depth == "deep" && all_items.len() >= 5 {
        let article_clusters = clustering::cluster_articles(all_items.clone(), 2);
        Some(
            article_clusters
                .into_iter()
                .take(20)
                .map(|c| ClusterInfo {
                    representative: c.representative,
                    member_count: c.members.len(),
                    entities: c.entities,
                    source_count: c.source_count,
                })
                .collect(),
        )
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

/// Full intelligence pipeline: fetch -> enrich -> index.
/// This merges the old `intelligence.collect` into `news.fetch@depth=deep`.
pub async fn fetch_news_intelligent(
    input: NewsFetchInput,
    insights: &Arc<Mutex<InsightStorage>>,
) -> Result<serde_json::Value, String> {
    // Use json format for internal pipeline steps
    let mut fetch_input = input.clone();
    fetch_input.output.format = Some("json".to_string());

    // Step 1: Fetch with regular news_fetch
    let fetch_output = news_fetch(fetch_input).await?;
    let fetched = fetch_output.count;
    let fetch_meta = fetch_output.meta;

    if fetched == 0 {
        let stats = insights.lock().await.stats();
        return Ok(serde_json::json!({
            "fetched": 0,
            "enriched": 0,
            "indexed": 0,
            "stats": stats,
            "fetch_meta": fetch_meta,
        }));
    }

    // Step 2: Enrich with NLP (unless skipped)
    let enriched_items: Vec<EnrichedItem> = if input.skip_enrich.unwrap_or(false) {
        fetch_output
            .items
            .iter()
            .map(|item| EnrichedItem {
                item: news_item_to_json(item),
                topics: Vec::new(),
                entities: Vec::new(),
                sentiment: None,
                summary: None,
            })
            .collect()
    } else {
        let enrich_input = NewsEnrichInput {
            items: fetch_output
                .items
                .iter()
                .map(|item| EnrichItemInput {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    link: item.link.clone(),
                    pub_date: item.pub_date.clone(),
                    source_name: item.source_name.clone(),
                    pool_id: item.pool_id.clone(),
                    content_snippet: Some(item.content_snippet.clone()),
                    date_confidence: item.date_confidence.clone(),
                    freshness_score: item.freshness_score,
                })
                .collect(),
            extract: None,
            output: OutputOptions {
                format: Some("json".to_string()),
            },
        };

        let enrich_output = news_enrich(enrich_input).await?;
        enrich_output.items
    };

    let enriched_count = enriched_items.len();

    // Step 3: Index in insight engine (unless skipped)
    let indexed_count = if input.skip_index.unwrap_or(false) {
        0
    } else {
        let articles: Vec<InsightIndexArticle> = enriched_items
            .iter()
            .filter_map(|item| {
                let id = item.item["id"].as_str()?.to_string();
                let title = item.item["title"].as_str()?.to_string();
                let pub_date = item.item["pub_date"].as_str()?.to_string();
                let source_name = item.item["source_name"].as_str()?.to_string();

                let entities = Some(item.entities.clone());

                let domains = item
                    .item
                    .get("pool_id")
                    .and_then(|p| p.as_str())
                    .map(|pool| {
                        vec![DomainInfo {
                            domain: pool.to_string(),
                            score: Some(1.0),
                        }]
                    });

                Some(InsightIndexArticle {
                    id,
                    title,
                    pub_date,
                    source_name,
                    domains,
                    entities,
                })
            })
            .collect();

        if articles.is_empty() {
            0
        } else {
            let index_input = InsightIndexInput { articles };
            let index_output =
                crate::tools::insights::insights_index(insights, index_input).await?;
            index_output.indexed
        }
    };

    let stats = insights.lock().await.stats();

    Ok(serde_json::json!({
        "fetched": fetched,
        "enriched": enriched_count,
        "indexed": indexed_count,
        "stats": stats,
        "fetch_meta": fetch_meta,
    }))
}

/// Debug helper. Test a single source and return up to 10 items.
pub async fn news_test_source(input: NewsTestInput) -> Result<NewsTestOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    let sf = config::load_sources()
        .await
        .map_err(|e| format!("Sources: {}", e))?;

    let src = sf
        .sources
        .iter()
        .find(|s| s.id == input.id)
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
    let extract = input.extract.unwrap_or_else(|| {
        vec![
            "topics".into(),
            "entities".into(),
            "sentiment".into(),
            "summary".into(),
        ]
    });
    let features = extract.clone();
    let want: std::collections::HashSet<String> = extract.into_iter().collect();

    let mut out = Vec::new();
    for item in &input.items {
        let text = format!(
            "{} {}",
            item.title,
            item.content_snippet.as_deref().unwrap_or("")
        );

        // Construct a temporary NewsItem to reuse news_item_to_json.
        // author and media_url are not part of the enriched output schema,
        // so they're set to None.
        let news_item = NewsItem {
            id: item.id.clone(),
            title: item.title.clone(),
            link: item.link.clone(),
            pub_date: item.pub_date.clone(),
            source_name: item.source_name.clone(),
            pool_id: item.pool_id.clone(),
            content_snippet: item.content_snippet.clone().unwrap_or_default(),
            author: None,
            media_url: None,
            date_confidence: item.date_confidence.clone(),
            freshness_score: item.freshness_score,
        };

        let mut enriched = EnrichedItem {
            item: news_item_to_json(&news_item),
            topics: Vec::new(),
            entities: Vec::new(),
            sentiment: None,
            summary: None,
        };

        if want.contains("topics") {
            enriched.topics = extract_topics(&text, 8);
        }

        if want.contains("entities") {
            enriched.entities = extract_basic_entities(&text)
                .into_iter()
                .filter_map(|v| {
                    Some(EntityInfo {
                        name: v["name"].as_str()?.to_string(),
                        entity_type: v["type"].as_str().unwrap_or("Unknown").to_string(),
                        mentions: v["mentions"].as_u64().map(|n| n as u32),
                        confidence: v["confidence"].as_f64(),
                        normalized_id: None,
                    })
                })
                .collect();
        }

        if want.contains("sentiment") {
            let sentiment_value = basic_sentiment(&text);
            enriched.sentiment = Some(SentimentResult {
                score: sentiment_value["score"].as_f64().unwrap_or(0.0),
                comparative: sentiment_value["comparative"].as_f64().unwrap_or(0.0),
                label: sentiment_value["label"]
                    .as_str()
                    .unwrap_or("neutral")
                    .to_string(),
            });
        }

        if want.contains("summary") {
            // "summary" is the first non-empty sentence of the content snippet,
            // falling back to the title if no snippet is available. This is a
            // naive heuristic — for production summarization, integrate an LLM.
            // The field is named "summary" in the output schema for forward
            // compatibility (a future LLM-based summary would populate the
            // same field without breaking clients).
            enriched.summary = item
                .content_snippet
                .as_deref()
                .and_then(|s| {
                    s.split(['.', '!', '?'])
                        .find(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_string())
                })
                .filter(|s| !s.is_empty())
                .or_else(|| Some(item.title.clone()));
        }

        if want.contains("diversity") {
            let title_words: Vec<String> = item
                .title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .map(|s| s.to_string())
                .collect();
            let same_source_count = input
                .items
                .iter()
                .filter(|other| {
                    other.id != item.id
                        && other.source_name != item.source_name
                        && title_words.iter().any(|tw| {
                            other
                                .title
                                .to_lowercase()
                                .split_whitespace()
                                .any(|ow| ow == tw)
                        })
                })
                .count();
            let total_sources: usize = input
                .items
                .iter()
                .map(|i| &i.source_name)
                .collect::<std::collections::HashSet<_>>()
                .len();
            let diversity = if total_sources <= 1 {
                0.0
            } else {
                (same_source_count as f64 / total_sources.max(1) as f64).min(1.0)
            };
            // diversity is not a field on EnrichedItem — keep as part of item value
            if let Some(obj) = enriched.item.as_object_mut() {
                obj.insert("source_diversity".to_string(), serde_json::json!(diversity));
            }
        }

        out.push(enriched);
    }

    Ok(NewsEnrichOutput {
        items: out,
        meta: EnrichmentMeta {
            enriched_count: input.items.len(),
            features,
        },
    })
}
