use std::collections::HashSet;
use crate::types::NewsItem;

pub struct ArticleCluster {
    pub representative: NewsItem,
    pub members: Vec<NewsItem>,
    pub entities: Vec<String>,
    pub source_count: usize,
}

fn extract_entities(text: &str) -> Vec<String> {
    let stop_words: HashSet<&str> = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "shall", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "before", "after", "above", "below", "between", "out", "off", "over",
        "under", "again", "further", "then", "once", "here", "there", "when",
        "where", "why", "how", "all", "each", "every", "both", "few", "more",
        "most", "other", "some", "such", "no", "nor", "not", "only", "own",
        "same", "so", "than", "too", "very", "just", "because", "but", "and",
        "or", "if", "while", "about", "against", "up", "down", "its", "it",
        "this", "that", "these", "those", "new", "says", "said", "also",
    ].iter().cloned().collect();

    text.split_whitespace()
        .filter(|word| {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
            clean.len() > 2
                && !stop_words.contains(clean.to_lowercase().as_str())
                && clean.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
        })
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .collect()
}

pub fn cluster_articles(items: Vec<NewsItem>, min_overlap: usize) -> Vec<ArticleCluster> {
    if items.is_empty() {
        return vec![];
    }

    let item_entities: Vec<(usize, Vec<String>)> = items.iter().enumerate().map(|(i, item)| {
        let text = format!("{} {}", item.title, item.content_snippet);
        (i, extract_entities(&text))
    }).collect();

    let mut assigned = vec![false; items.len()];
    let mut clusters = Vec::new();

    for (i, entities_i) in &item_entities {
        if assigned[*i] {
            continue;
        }

        let mut cluster_indices = vec![*i];
        let cluster_entities: HashSet<String> = entities_i.iter().cloned().collect();

        for (j, entities_j) in &item_entities {
            if *j <= *i || assigned[*j] {
                continue;
            }

            let overlap = entities_i.iter().filter(|e| entities_j.contains(*e)).count();
            if overlap >= min_overlap {
                cluster_indices.push(*j);
            }
        }

        for &idx in &cluster_indices {
            assigned[idx] = true;
        }

        let members: Vec<NewsItem> = cluster_indices.iter()
            .map(|&idx| items[idx].clone())
            .collect();

        let source_count = members.iter()
            .map(|m| &m.source_name)
            .collect::<HashSet<_>>()
            .len();

        let representative = members.iter()
            .max_by_key(|m| m.freshness_score.map(|s| (s * 1000.0) as i64).unwrap_or(0))
            .unwrap_or(&members[0])
            .clone();

        clusters.push(ArticleCluster {
            representative,
            members,
            entities: cluster_entities.into_iter().collect(),
            source_count,
        });
    }

    clusters.sort_by(|a, b| b.source_count.cmp(&a.source_count)
        .then(b.members.len().cmp(&a.members.len())));

    clusters
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(title: &str, source: &str) -> NewsItem {
        NewsItem {
            id: format!("{}-{}", source, title.len()),
            title: title.to_string(),
            link: format!("https://example.com/{}", title.replace(' ', "-")),
            pub_date: "2026-01-01T00:00:00Z".to_string(),
            source_name: source.to_string(),
            pool_id: "TEST".to_string(),
            content_snippet: String::new(),
            author: None,
            media_url: None,
            date_confidence: Some("high".to_string()),
            freshness_score: Some(80.0),
        }
    }

    #[test]
    fn test_cluster_same_entity() {
        let items = vec![
            make_item("Apple announces new iPhone", "reuters"),
            make_item("Apple launches iPhone 16", "techcrunch"),
            make_item("Weather update today", "weather_com"),
        ];
        let clusters = cluster_articles(items, 1);
        assert!(clusters.len() <= 2);
    }

    #[test]
    fn test_cluster_empty() {
        let clusters = cluster_articles(vec![], 1);
        assert!(clusters.is_empty());
    }
}
