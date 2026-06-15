use std::collections::HashMap;
use crate::types::NewsItem;

/// Generate a dedup key from a NewsItem's title for RRF fusion.
/// Uses normalized title words (lowercased, stripped of short words).
fn dedup_key(item: &NewsItem) -> String {
    let mut words: Vec<String> = item.title
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect();
    words.sort();
    words.join(" ")
}

/// Weighted Reciprocal Rank Fusion (RRF).
///
/// Combines multiple ranked lists from different sources into a single
/// ranked list. Items appearing in multiple sources get higher scores.
///
/// # Arguments
/// * `result_lists` - Vec of (items, source_weight) tuples. Items should be
///   in relevance order (most relevant first). Source weight is typically 1.0
///   but can be adjusted via the Source.weight config field.
/// * `k` - RRF constant (default 60). Higher k reduces the impact of rank
///   differences. Standard value is 60.
///
/// # Returns
/// Items sorted by RRF score (highest first), deduplicated by title.
pub fn weighted_rrf_fusion(
    result_lists: Vec<(Vec<NewsItem>, f64)>,
    k: usize,
) -> Vec<NewsItem> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut item_map: HashMap<String, NewsItem> = HashMap::new();

    for (items, weight) in result_lists {
        for (rank, item) in items.iter().enumerate() {
            let key = dedup_key(item);
            if key.is_empty() {
                continue;
            }
            let rrf_score = 1.0 / (k + rank + 1) as f64;
            *scores.entry(key.clone()).or_insert(0.0) += weight * rrf_score;
            // Keep the first (most relevant) item for each key
            item_map.entry(key).or_insert_with(|| item.clone());
        }
    }

    let mut scored: Vec<(String, f64)> = scores.into_iter().collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored.into_iter()
        .filter_map(|(key, _)| item_map.remove(&key))
        .collect()
}

/// Compute source diversity score for an item based on how many sources
/// report the same story. Returns a value between 0.0 (single source)
/// and 1.0 (many sources).
pub fn source_diversity(item: &NewsItem, all_sources: &[String]) -> f64 {
    if all_sources.len() <= 1 {
        return 0.0;
    }
    // Count how many sources mention the same keywords from the title
    let title_words: Vec<String> = item.title
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .map(|s| s.to_string())
        .collect();

    if title_words.is_empty() {
        return 0.0;
    }

    let matching = all_sources.iter().filter(|source| {
        source.to_lowercase().split_whitespace().any(|w| {
            title_words.iter().any(|tw| tw == w)
        })
    }).count();

    (matching as f64 / all_sources.len().max(1) as f64).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NewsItem;

    fn make_item(title: &str, source: &str) -> NewsItem {
        NewsItem {
            id: format!("{}-{}", source, title.len()),
            title: title.to_string(),
            link: format!("https://example.com/{}", title.replace(' ', "-")),
            pub_date: "2026-01-01T00:00:00Z".to_string(),
            source_name: source.to_string(),
            pool_id: "TEST".to_string(),
            content_snippet: format!("Content for {}", title),
            author: None,
            media_url: None,
            date_confidence: Some("high".to_string()),
            freshness_score: Some(80.0),
        }
    }

    #[test]
    fn test_rrf_single_source() {
        let items = vec![
            make_item("Breaking news today", "reuters"),
            make_item("Tech stocks rise", "techcrunch"),
        ];
        let result = weighted_rrf_fusion(vec![(items, 1.0)], 60);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_rrf_cross_source_ranking() {
        // Same story from two sources should rank higher
        let source_a = vec![
            make_item("AI breakthrough announced", "reuters"),
            make_item("Weather update", "reuters"),
        ];
        let source_b = vec![
            make_item("AI breakthrough announced", "techcrunch"),
            make_item("New phone released", "techcrunch"),
        ];
        let result = weighted_rrf_fusion(vec![(source_a, 1.0), (source_b, 1.0)], 60);
        // AI story should be first (appears in 2 sources)
        assert!(result[0].title.contains("AI breakthrough"));
    }

    #[test]
    fn test_rrf_with_weights() {
        let source_a = vec![make_item("Story A", "blog")]; // low weight
        let source_b = vec![make_item("Story A", "reuters")]; // high weight
        let result = weighted_rrf_fusion(vec![
            (source_a, 0.3),
            (source_b, 2.0),
        ], 60);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Story A");
    }
}
