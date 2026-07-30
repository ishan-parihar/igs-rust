// Re-export shared toon-helper functions
pub use toon_helper::{toon_encode, format_text, truncate_str, truncate_json_strings, print_output};

// Re-export NLP utilities for backward compatibility
pub use super::nlp::{extract_topics, extract_basic_entities, basic_sentiment};

/// URL-encode a string
pub fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Sync helper: find RSS/Atom feed link in HTML body
pub fn find_feed_url(body: &str, base_url: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(body);
    if let Ok(sel) = scraper::Selector::parse(
        "link[rel='alternate'][type*='rss'], link[rel='alternate'][type*='atom']",
    ) {
        for el in doc.select(&sel) {
            if let Some(href) = el.attr("href") {
                let abs = url::Url::parse(base_url)
                    .ok()
                    .and_then(|base| url::Url::parse(href).ok().or_else(|| base.join(href).ok()))
                    .map(|u| u.to_string());
                if abs.is_some() {
                    return abs;
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let names: Vec<String> = entities
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"John Smith".to_string()));
    }

    #[test]
    fn test_extract_basic_entities_organization() {
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
