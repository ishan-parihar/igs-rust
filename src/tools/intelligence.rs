use crate::server::InsightStorage;
use crate::tools::news;
use crate::tools::types::*;
use crate::types::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Intelligence pipeline: fetch → enrich → index in one call
pub async fn intelligence_collect(
    insights: &Arc<Mutex<InsightStorage>>,
    input: IntelligenceCollectInput,
) -> Result<IntelligenceCollectOutput, String> {
    // Step 1: Fetch news
    let fetch_input = NewsFetchInput {
        pools: input.pools,
        sources: input.sources,
        countries: input.countries,
        cities: input.cities,
        domains: input.domains,
        start: input.start,
        end: input.end,
        keywords: input.keywords,
        exclude_keywords: input.exclude_keywords,
        match_all: input.match_all,
        discovery_mode: None,
        limit: input.limit,
        cache_mode: input.cache_mode,
        urgency: None,
        format: Some("json".to_string()),
        depth: input.depth,
    };

    let fetch_output = news::news_fetch(fetch_input).await?;
    let fetched = fetch_output.count;
    let fetch_meta = fetch_output.meta;

    if fetched == 0 {
        let stats = insights.lock().await.stats();
        return Ok(IntelligenceCollectOutput {
            fetched: 0,
            enriched: 0,
            indexed: 0,
            stats,
            fetch_meta,
        });
    }

    // Step 2: Enrich with NLP (unless skipped)
    let enriched_items = if input.skip_enrich.unwrap_or(false) {
        fetch_output.items.iter().map(|item| {
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
        }).collect::<Vec<_>>()
    } else {
        let enrich_input = NewsEnrichInput {
            items: fetch_output.items.iter().map(|item| EnrichItemInput {
                id: item.id.clone(),
                title: item.title.clone(),
                link: item.link.clone(),
                pub_date: item.pub_date.clone(),
                source_name: item.source_name.clone(),
                pool_id: item.pool_id.clone(),
                content_snippet: Some(item.content_snippet.clone()),
                date_confidence: item.date_confidence.clone(),
                freshness_score: item.freshness_score,
            }).collect(),
            extract: None,
            format: Some("json".to_string()),
        };

        let enrich_output = news::news_enrich(enrich_input).await?;
        enrich_output.items
    };

    let enriched_count = enriched_items.len();

    // Step 3: Index in insight engine (unless skipped)
    let indexed_count = if input.skip_index.unwrap_or(false) {
        0
    } else {
        let articles: Vec<InsightIndexArticle> = enriched_items.iter().filter_map(|item| {
            let id = item["id"].as_str()?.to_string();
            let title = item["title"].as_str()?.to_string();
            let pub_date = item["pub_date"].as_str()?.to_string();
            let source_name = item["source_name"].as_str()?.to_string();

            // Extract entities from enriched data
            let entities = item.get("entities").and_then(|e| e.as_array()).map(|arr| {
                arr.iter().filter_map(|e| {
                    Some(EntityInfo {
                        name: e["name"].as_str()?.to_string(),
                        entity_type: e["type"].as_str().unwrap_or("Unknown").to_string(),
                        mentions: e["mentions"].as_u64().map(|n| n as u32),
                        confidence: e["confidence"].as_f64(),
                        normalized_id: None,
                    })
                }).collect()
            });

            // Extract domains from pool_id
            let domains = item.get("pool_id").and_then(|p| p.as_str()).map(|pool| {
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
        }).collect();

        if articles.is_empty() {
            0
        } else {
            let index_input = InsightIndexInput { articles };
            let index_output = crate::tools::insights::insights_index(insights, index_input).await?;
            index_output.indexed
        }
    };

    let stats = insights.lock().await.stats();

    Ok(IntelligenceCollectOutput {
        fetched,
        enriched: enriched_count,
        indexed: indexed_count,
        stats,
        fetch_meta,
    })
}
