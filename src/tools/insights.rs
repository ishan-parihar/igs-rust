use crate::server::InsightStorage;
use crate::tools::types::*;
use crate::tools::types_base::OutputOptions;
use crate::types::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Unified connection finder: specific entity OR all cross-domain entities
pub(crate) async fn insights_find_connections(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightFindConnectionsInput,
) -> Result<InsightFindConnectionsOutput, String> {
    let storage = storage.lock().await;
    let min_domains = input.min_domains.unwrap_or(2) as usize;

    if let Some(ref entity) = input.entity {
        // Specific entity lookup
        let connections = storage.find_inter_domain_connections(entity, min_domains);
        let count = connections.len();
        Ok(InsightFindConnectionsOutput { connections, count, total_found: None, stats: None })
    } else {
        // All cross-domain entities
        let all = storage.find_all_inter_domain_connections(min_domains);
        let total_found = all.len();
        let limit = input.limit.unwrap_or(20) as usize;
        let connections: Vec<EntityConnection> = all.into_iter().take(limit).collect();
        let count = connections.len();
        let stats = storage.stats();
        Ok(InsightFindConnectionsOutput { connections, count, total_found: Some(total_found), stats: Some(stats) })
    }
}

/// [DEPRECATED] Use insights_find_connections with entity=Some(...)
pub(crate) async fn insights_find_connections_entity(
    storage: &Arc<Mutex<InsightStorage>>,
    entity: String,
    min_domains: Option<i32>,
) -> Result<InsightConnectionOutput, String> {
    let input = InsightFindConnectionsInput { entity: Some(entity), min_domains, limit: None, output: OutputOptions { format: None } };
    let result = insights_find_connections(storage, input).await?;
    Ok(InsightConnectionOutput { connections: result.connections, count: result.count })
}

/// [DEPRECATED] Use insights_find_connections with entity=None
pub(crate) async fn insights_find_all_connections_legacy(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightAllConnectionsInput,
) -> Result<InsightAllConnectionsOutput, String> {
    let unified_input = InsightFindConnectionsInput {
        entity: None,
        min_domains: input.min_domains,
        limit: input.limit,
        output: input.output,
    };
    let result = insights_find_connections(storage, unified_input).await?;
    Ok(InsightAllConnectionsOutput {
        connections: result.connections,
        total_found: result.total_found.unwrap_or(0),
        stats: result.stats.unwrap_or(InsightStats { total_articles: 0, total_entities: 0, total_domains: 0, avg_entities_per_article: 0.0, avg_domains_per_article: 0.0 }),
    })
}

/// Detect entities with increasing mention frequency
pub(crate) async fn insights_trending(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightTrendingInput,
) -> Result<InsightTrendingOutput, String> {
    let storage = storage.lock().await;
    let window_ms = input.time_window_hours.unwrap_or(24) * 3_600_000;
    let trending = storage.detect_trending(
        window_ms,
        input.min_growth.unwrap_or(2.0),
        input.min_current_mentions.unwrap_or(3),
    );
    let count = trending.len();
    let stats = storage.stats();
    Ok(InsightTrendingOutput { trending, count, stats })
}

/// Add articles to the insight engine for cross-article analysis
pub(crate) async fn insights_index(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightIndexInput,
) -> Result<InsightIndexOutput, String> {
    let mut storage = storage.lock().await;
    let mut indexed = 0usize;

    for article in &input.articles {
        storage.add_article(ArticleInsight {
            id: article.id.clone(),
            title: article.title.clone(),
            pub_date: article.pub_date.clone(),
            source_name: article.source_name.clone(),
            domains: article.domains.clone().unwrap_or_default(),
            entities: article.entities.clone().unwrap_or_default(),
        });
        indexed += 1;
    }

    let stats = storage.stats();
    Ok(InsightIndexOutput { indexed, stats })
}

/// Get statistics about indexed articles
pub(crate) async fn insights_stats(
    storage: &Arc<Mutex<InsightStorage>>,
) -> Result<InsightStatsOutput, String> {
    let storage = storage.lock().await;
    let stats = storage.stats();
    Ok(InsightStatsOutput { stats })
}

/// Clear all indexed articles from the insight engine
pub(crate) async fn insights_clear(
    storage: &Arc<Mutex<InsightStorage>>,
) -> Result<InsightClearOutput, String> {
    let mut storage = storage.lock().await;
    storage.clear();
    Ok(InsightClearOutput { cleared: true })
}
