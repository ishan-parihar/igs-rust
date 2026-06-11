use std::collections::HashMap;

/// URL-encode a string
pub fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Basic topic extraction via word frequency
pub fn extract_topics(text: &str, max: usize) -> Vec<String> {
    let lower = text.to_lowercase();
    let stop_words = [
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "by", "with", "from", "is", "are", "was", "were", "be", "been",
        "being", "have", "has", "had", "do", "does", "did", "will", "would",
        "could", "should", "may", "might", "shall", "can", "need", "dare",
        "it", "its", "it's", "this", "that", "these", "those", "i", "you",
        "he", "she", "we", "they", "me", "him", "her", "us", "them", "my",
        "your", "his", "its", "our", "their", "not", "no", "nor", "so", "up",
        "down", "out", "off", "over", "under", "again", "further", "then",
        "once", "here", "there", "when", "where", "why", "how", "all", "each",
        "every", "both", "few", "more", "most", "other", "some", "such", "only",
        "own", "same", "than", "too", "very", "just", "also", "about",
    ];

    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3 && !stop_words.contains(w))
        .collect();

    let mut freq: HashMap<&str, usize> = HashMap::new();
    for w in &words {
        *freq.entry(w).or_default() += 1;
    }

    let mut topics: Vec<(&str, usize)> = freq.into_iter().collect();
    topics.sort_by(|a, b| b.1.cmp(&a.1));
    topics.into_iter().take(max).map(|(w, _)| w.to_string()).collect()
}

/// Basic entity extraction
pub fn extract_basic_entities(text: &str) -> Vec<serde_json::Value> {
    let mut entities = Vec::new();

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i].trim_matches(|c: char| !c.is_alphanumeric());
        if w.len() >= 2 && w.chars().next().is_some_and(|c| c.is_uppercase())
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
            if !entities.iter().any(|e: &serde_json::Value| e["name"] == name) {
                let entity_type = if name.contains(' ') { "Person" } else { "Organization" };
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

/// Basic sentiment analysis
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
        "damage", "threat", "risk", "danger", "dangerous", "war", "conflict", "attack",
        "crime", "illegal", "corruption", "scandal", "fraud", "banned", "restrict",
        "regret", "sad", "angry", "furious", "tragic", "deadly", "fatal", "kill",
        "death", "destroy", "destruction", "hate", "wrong", "ugly", "harsh",
    ];

    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    let pos_count = words.iter().filter(|w| positive_words.contains(w)).count();
    let neg_count = words.iter().filter(|w| negative_words.contains(w)).count();

    let score = (pos_count as f64) - (neg_count as f64);
    let total = words.len() as f64;
    let comparative = if total > 0.0 { score / total } else { 0.0 };
    let label = if score > 0.0 { "positive" } else if score < 0.0 { "negative" } else { "neutral" };

    serde_json::json!({
        "score": score,
        "comparative": comparative,
        "label": label,
    })
}

/// Sync helper: find RSS/Atom feed link in HTML body
pub fn find_feed_url(body: &str, base_url: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(body);
    if let Ok(sel) = scraper::Selector::parse("link[rel='alternate'][type*='rss'], link[rel='alternate'][type*='atom']") {
        for el in doc.select(&sel) {
            if let Some(href) = el.attr("href") {
                let abs = url::Url::parse(base_url).ok().and_then(|base| {
                    url::Url::parse(href).ok().or_else(|| base.join(href).ok())
                }).map(|u| u.to_string());
                if abs.is_some() {
                    return abs;
                }
            }
        }
    }
    None
}

/// TOON-format encode a serializable value for AI-agent token-efficiency
pub fn toon_encode<T: serde::Serialize>(value: &T) -> String {
    toon_format::encode_default(value).unwrap_or_else(|_| serde_json::to_string(value).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toon_encode_simple_object() {
        let value = serde_json::json!({"name": "Alice", "age": 30});
        let result = toon_encode(&value);
        assert!(!result.is_empty());
        // TOON format should not start with JSON brace
        assert!(!result.starts_with('{'));
    }

    #[test]
    fn test_toon_encode_array() {
        let value = serde_json::json!([1, 2, 3]);
        let result = toon_encode(&value);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_toon_encode_fallback() {
        // Test with a value that might fail TOON encoding
        let value = serde_json::json!(null);
        let result = toon_encode(&value);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_extract_topics_basic() {
        let text = "machine learning algorithms are used in artificial intelligence applications and machine learning models";
        let topics = extract_topics(text, 3);
        assert!(!topics.is_empty());
        assert!(topics.contains(&"machine".to_string()));
        assert!(topics.contains(&"learning".to_string()));
    }

    #[test]
    fn test_extract_topics_stop_words() {
        let text = "the a an and or but in on at to for of by with from is are was were";
        let topics = extract_topics(text, 5);
        // All stop words should be filtered out
        assert!(topics.is_empty());
    }

    #[test]
    fn test_extract_topics_max_limit() {
        let text = "one two three four five six seven eight nine ten";
        let topics = extract_topics(text, 3);
        assert!(topics.len() <= 3);
    }

    #[test]
    fn test_extract_basic_entities_person() {
        let text = "John Smith went to New York City";
        let entities = extract_basic_entities(text);
        assert!(!entities.is_empty());
        let names: Vec<String> = entities.iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"John Smith".to_string()));
    }

    #[test]
    fn test_extract_basic_entities_organization() {
        let text = "Microsoft released a new product";
        let entities = extract_basic_entities(text);
        assert!(!entities.is_empty());
        let names: Vec<String> = entities.iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"Microsoft".to_string()));
    }

    #[test]
    fn test_basic_sentiment_positive() {
        let text = "This is a great and amazing success with wonderful progress";
        let sentiment = basic_sentiment(text);
        assert_eq!(sentiment["label"], "positive");
        assert!(sentiment["score"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn test_basic_sentiment_negative() {
        let text = "This is a terrible disaster with horrible failure and tragic loss";
        let sentiment = basic_sentiment(text);
        assert_eq!(sentiment["label"], "negative");
        assert!(sentiment["score"].as_f64().unwrap() < 0.0);
    }

    #[test]
    fn test_basic_sentiment_neutral() {
        let text = "The weather is cloudy today";
        let sentiment = basic_sentiment(text);
        assert_eq!(sentiment["label"], "neutral");
        assert_eq!(sentiment["score"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_urlencoding() {
        let encoded = urlencoding("hello world&foo=bar");
        assert_eq!(encoded, "hello+world%26foo%3Dbar");
    }

    #[test]
    fn test_urlencoding_empty() {
        let encoded = urlencoding("");
        assert_eq!(encoded, "");
    }
}

