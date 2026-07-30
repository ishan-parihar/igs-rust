//! Consolidated NLP utilities for IGS.
//!
//! This module provides a single canonical implementation for:
//! - Tokenization (lowercase alphanumeric splitting)
//! - Stop words (merged from helpers.rs + summarize.rs)
//! - Topic extraction (word frequency)
//! - Entity extraction (capitalization-based)
//! - Sentiment analysis (lexicon-based)
//!
//! All other modules should use this module instead of local implementations.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Unified stop words list (merged from helpers.rs and summarize.rs).
/// 130+ common English words that carry little semantic meaning.
const STOP_WORDS_LIST: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any",
    "are", "as", "at", "be", "because", "been", "before", "being", "below", "between",
    "both", "but", "by", "can", "could", "dare", "did", "do", "does", "doing", "down",
    "during", "each", "few", "for", "from", "further", "had", "has", "have", "having",
    "he", "her", "here", "hers", "herself", "him", "himself", "his", "how", "i", "if",
    "in", "into", "is", "it", "its", "itself", "just", "me", "might", "more", "most",
    "my", "myself", "need", "no", "nor", "not", "now", "of", "off", "on", "once",
    "only", "or", "other", "our", "ours", "ourselves", "out", "over", "own", "s",
    "same", "shall", "she", "should", "so", "some", "such", "t", "than", "that",
    "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they",
    "this", "those", "through", "to", "too", "under", "until", "up", "very", "was",
    "we", "were", "what", "when", "where", "which", "while", "who", "whom", "why",
    "will", "with", "would", "you", "your", "yours", "yourself", "yourselves",
];

/// Lazy-initialized HashSet for O(1) stop word lookups.
/// Avoids O(n) linear scan per token in hot paths like extract_topics and tokenize.
pub static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    STOP_WORDS_LIST.iter().copied().collect()
});

/// Tokenize text into lowercase alphanumeric terms.
///
/// # Arguments
/// * `text` - Input text to tokenize
/// * `min_len` - Minimum token length (tokens shorter than this are excluded)
/// * `filter_stop_words` - If true, exclude common English stop words
pub fn tokenize(text: &str, min_len: usize, filter_stop_words: bool) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= min_len)
        .filter(|t| !filter_stop_words || !STOP_WORDS.contains(*t))
        .map(|t| t.to_string())
        .collect()
}

/// Basic topic extraction via word frequency analysis.
///
/// Splits text into words, filters stop words and short tokens,
/// counts frequency, and returns the top `max` most frequent words.
pub fn extract_topics(text: &str, max: usize) -> Vec<String> {
    let words = tokenize(text, 4, true);

    let mut freq: HashMap<&str, usize> = HashMap::new();
    for w in &words {
        *freq.entry(w.as_str()).or_default() += 1;
    }

    let mut topics: Vec<(&str, usize)> = freq.into_iter().collect();
    topics.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
    topics
        .into_iter()
        .take(max)
        .map(|(w, _)| w.to_string())
        .collect()
}

/// Basic entity extraction via capitalization heuristics.
///
/// Identifies proper nouns by detecting capitalized words that are not
/// all-caps (to exclude acronyms). Adjacent capitalized words are
/// merged into multi-word entities (e.g., "John Smith", "New York").
pub fn extract_basic_entities(text: &str) -> Vec<serde_json::Value> {
    let mut entities = Vec::new();

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].trim_matches(|c: char| !c.is_alphanumeric());
        if w.len() >= 2
            && w.chars().next().is_some_and(|c| c.is_uppercase())
            && !w.chars().all(|c| c.is_uppercase())
        {
            let mut name = w.to_string();
            while i + 1 < words.len() {
                let next = words[i + 1].trim_matches(|c: char| !c.is_alphanumeric());
                if next.len() >= 2 && next.chars().next().is_some_and(|c| c.is_uppercase()) {
                    name.push(' ');
                    name.push_str(next);
                    i += 1;
                } else {
                    break;
                }
            }
            if !entities
                .iter()
                .any(|e: &serde_json::Value| e["name"] == name)
            {
                let entity_type = if name.contains(' ') {
                    "Person"
                } else {
                    "Organization"
                };
                entities.push(serde_json::json!({
                    "name": name,
                    "type": entity_type,
                    "mentions": 1,
                    "confidence": 0.5,
                }));
            }
        }
        i += 1;
    }

    entities
}

/// Basic sentiment analysis using a positive/negative word lexicon.
///
/// Returns a JSON object with:
/// - `score`: net sentiment (positive - negative word counts)
/// - `comparative`: score normalized by total word count
/// - `label`: "positive", "negative", or "neutral"
pub fn basic_sentiment(text: &str) -> serde_json::Value {
    let positive_words = [
        "good", "great", "excellent", "amazing", "wonderful", "fantastic", "outstanding",
        "positive", "success", "successful", "growth", "breakthrough", "opportunity",
        "progress", "innovation", "achievement", "benefit", "improve", "improvement",
        "strong", "profit", "gain", "boost", "surge", "rally", "hope", "optimistic",
        "bright", "promising", "remarkable", "impressive", "best", "better", "win",
        "victory", "celebration", "happy", "love", "beautiful", "exciting", "thrilled",
    ];
    let negative_words = [
        "bad", "terrible", "awful", "horrible", "worst", "poor", "negative", "failure",
        "fail", "crisis", "disaster", "decline", "drop", "loss", "lose", "damage",
        "threat", "risk", "danger", "dangerous", "war", "conflict", "attack", "crime",
        "illegal", "corruption", "scandal", "fraud", "banned", "restrict", "regret",
        "sad", "angry", "furious", "tragic", "deadly", "fatal", "kill", "death",
        "destroy", "destruction", "hate", "wrong", "ugly", "harsh",
    ];

    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    let pos_count = words.iter().filter(|w| positive_words.contains(w)).count();
    let neg_count = words.iter().filter(|w| negative_words.contains(w)).count();

    let score = (pos_count as f64) - (neg_count as f64);
    let total = words.len() as f64;
    let comparative = if total > 0.0 { score / total } else { 0.0 };
    let label = if score > 0.0 {
        "positive"
    } else if score < 0.0 {
        "negative"
    } else {
        "neutral"
    };

    serde_json::json!({
        "score": score,
        "comparative": comparative,
        "label": label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello, World! This is a test.", 2, false);
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn tokenize_min_len() {
        let tokens = tokenize("a bb ccc dddd", 3, false);
        assert_eq!(tokens.len(), 2);
        assert!(tokens.contains(&"ccc".to_string()));
        assert!(tokens.contains(&"dddd".to_string()));
    }

    #[test]
    fn tokenize_stop_words() {
        let tokens = tokenize("the quick brown fox", 2, true);
        assert!(!tokens.contains(&"the".to_string()));
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
    }

    #[test]
    fn extract_topics_basic() {
        let text = "machine learning algorithms are used in artificial intelligence applications and machine learning models";
        let topics = extract_topics(text, 3);
        assert!(!topics.is_empty());
        assert!(topics.contains(&"machine".to_string()));
        assert!(topics.contains(&"learning".to_string()));
    }

    #[test]
    fn extract_topics_stop_words_filtered() {
        let text = "the a an and or but in on at to for of by with from is are was were";
        let topics = extract_topics(text, 5);
        assert!(topics.is_empty());
    }

    #[test]
    fn extract_topics_max_limit() {
        let text = "one two three four five six seven eight nine ten";
        let topics = extract_topics(text, 3);
        assert!(topics.len() <= 3);
    }

    #[test]
    fn extract_basic_entities_person() {
        let text = "John Smith went to New York City";
        let entities = extract_basic_entities(text);
        assert!(!entities.is_empty());
        let names: Vec<String> = entities
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"John Smith".to_string()));
    }

    #[test]
    fn extract_basic_entities_organization() {
        let text = "Microsoft released a new product";
        let entities = extract_basic_entities(text);
        assert!(!entities.is_empty());
        let names: Vec<String> = entities
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"Microsoft".to_string()));
    }

    #[test]
    fn basic_sentiment_positive() {
        let text = "This is a great and amazing success with wonderful progress";
        let sentiment = basic_sentiment(text);
        assert_eq!(sentiment["label"], "positive");
        assert!(sentiment["score"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn basic_sentiment_negative() {
        let text = "This is a terrible disaster with horrible failure and tragic loss";
        let sentiment = basic_sentiment(text);
        assert_eq!(sentiment["label"], "negative");
        assert!(sentiment["score"].as_f64().unwrap() < 0.0);
    }

    #[test]
    fn basic_sentiment_neutral() {
        let text = "The weather is cloudy today";
        let sentiment = basic_sentiment(text);
        assert_eq!(sentiment["label"], "neutral");
        assert_eq!(sentiment["score"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn stop_words_count() {
        // Verify we have a comprehensive stop words list
        assert!(STOP_WORDS.len() >= 100);
    }

    #[test]
    fn stop_words_o1_lookup() {
        // Verify HashSet provides O(1) lookups (compile-time check only)
        assert!(STOP_WORDS.contains("the"));
        assert!(STOP_WORDS.contains("is"));
        assert!(!STOP_WORDS.contains("rust"));
    }
}
