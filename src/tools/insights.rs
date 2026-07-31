use crate::server::InsightStorage;
use crate::tools::types::*;
use crate::types::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::error::AppResult;

/// Unified connection finder: specific entity OR all cross-domain entities
pub async fn insights_find_connections(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightFindConnectionsInput,
) -> AppResult<InsightFindConnectionsOutput> {
    let snapshot = storage.lock().await.snapshot();
    let min_domains = input.min_domains.unwrap_or(2) as usize;

    if let Some(ref entity) = input.entity {
        let connections = InsightStorage::find_inter_domain_connections_snapshot(&snapshot, entity, min_domains);
        let count = connections.len();
        Ok(InsightFindConnectionsOutput {
            connections,
            count,
            total_found: None,
            stats: None,
        })
    } else {
        let all = InsightStorage::find_all_inter_domain_connections_snapshot(&snapshot, min_domains);
        let total_found = all.len();
        let limit = input.limit.unwrap_or(20) as usize;
        let connections: Vec<EntityConnection> = all.into_iter().take(limit).collect();
        let count = connections.len();
        let stats = InsightStorage::stats_snapshot(&snapshot);
        Ok(InsightFindConnectionsOutput {
            connections,
            count,
            total_found: Some(total_found),
            stats: Some(stats),
        })
    }
}

/// Detect entities with increasing mention frequency
pub async fn insights_trending(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightTrendingInput,
) -> AppResult<InsightTrendingOutput> {
    let snapshot = storage.lock().await.snapshot();
    let window_ms = input.time_window_hours.unwrap_or(24) * 3_600_000;
    let trending = InsightStorage::detect_trending_snapshot(
        &snapshot,
        window_ms,
        input.min_growth.unwrap_or(2.0),
        input.min_current_mentions.unwrap_or(3),
    );
    let count = trending.len();
    let stats = InsightStorage::stats_snapshot(&snapshot);
    Ok(InsightTrendingOutput {
        trending,
        count,
        stats,
    })
}

/// Add articles to the insight engine for cross-article analysis
pub async fn insights_index(
    storage: &Arc<Mutex<InsightStorage>>,
    input: InsightIndexInput,
) -> AppResult<InsightIndexOutput> {
    let mut storage = storage.lock().await;
    let indexed = input.articles.len();

    let articles: Vec<ArticleInsight> = input
        .articles
        .iter()
        .map(|a| ArticleInsight {
            id: a.id.clone(),
            title: a.title.clone(),
            pub_date: a.pub_date.clone(),
            source_name: a.source_name.clone(),
            domains: a.domains.clone().unwrap_or_default(),
            entities: a.entities.clone().unwrap_or_default(),
        })
        .collect();

    storage.add_articles_batch(articles);

    let stats = storage.stats();
    Ok(InsightIndexOutput { indexed, stats })
}

/// Get statistics about indexed articles
pub async fn insights_stats(
    storage: &Arc<Mutex<InsightStorage>>,
) -> AppResult<InsightStatsOutput> {
    let snapshot = storage.lock().await.snapshot();
    let stats = InsightStorage::stats_snapshot(&snapshot);
    Ok(InsightStatsOutput { stats })
}

/// Clear all indexed articles from the insight engine
pub async fn insights_clear(
    storage: &Arc<Mutex<InsightStorage>>,
) -> AppResult<InsightClearOutput> {
    let mut storage = storage.lock().await;
    storage.clear();
    Ok(InsightClearOutput { cleared: true })
}
