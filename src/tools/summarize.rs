//! Extractive summarization using TextRank algorithm.
//!
//! TextRank is a graph-based ranking algorithm for natural language processing:
//! 1. Split text into sentences
//! 2. Build a similarity graph (sentences are nodes, similarity scores are edges)
//! 3. Run PageRank on the graph to rank sentences
//! 4. Return the top-N sentences as the summary
//!
//! This is a pure-Rust implementation with no external API calls.
//! The similarity metric is cosine similarity on bag-of-words vectors.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryResult {
    pub summary: String,
    pub sentence_count: usize,
    pub original_count: usize,
    pub top_sentences: Vec<String>,
}

const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and",
    "any", "are", "as", "at", "be", "because", "been", "before", "being", "below",
    "between", "both", "but", "by", "can", "did", "do", "does", "doing", "down",
    "during", "each", "few", "for", "from", "further", "had", "has", "have",
    "having", "he", "her", "here", "hers", "herself", "him", "himself", "his",
    "how", "i", "if", "in", "into", "is", "it", "its", "itself", "just", "me",
    "more", "most", "my", "myself", "no", "nor", "not", "now", "of", "off", "on",
    "once", "only", "or", "other", "our", "ours", "ourselves", "out", "over",
    "own", "s", "same", "she", "should", "so", "some", "such", "t", "than",
    "that", "the", "their", "theirs", "them", "themselves", "then", "there",
    "these", "they", "this", "those", "through", "to", "too", "under", "until",
    "up", "very", "was", "we", "were", "what", "when", "where", "which", "while",
    "who", "whom", "why", "will", "with", "would", "you", "your", "yours",
    "yourself", "yourselves",
];

/// Split text into sentences using common delimiters.
fn split_sentences(text: &str) -> Vec<String> {
    text.split(|c: char| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim())
        .filter(|s| s.len() > 20) // skip very short fragments
        .map(|s| s.to_string())
        .collect()
}

/// Tokenize a sentence into lowercase words, filtering stop words.
fn tokenize(sentence: &str) -> Vec<String> {
    sentence
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Build a bag-of-words vector for a sentence.
fn bow_vector(tokens: &[String]) -> HashMap<String, f64> {
    let mut vec = HashMap::new();
    for token in tokens {
        *vec.entry(token.clone()).or_insert(0.0) += 1.0;
    }
    // Normalize by magnitude
    let magnitude = vec.values().map(|v| v * v).sum::<f64>().sqrt();
    if magnitude > 0.0 {
        for v in vec.values_mut() {
            *v /= magnitude;
        }
    }
    vec
}

/// Compute cosine similarity between two bag-of-words vectors.
fn cosine_similarity(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
    let mut dot = 0.0;
    for (key, val_a) in a {
        if let Some(val_b) = b.get(key) {
            dot += val_a * val_b;
        }
    }
    dot // already normalized
}

/// Run PageRank on the sentence similarity graph.
fn pagerank(
    similarity: &[Vec<f64>],
    damping: f64,
    iterations: usize,
) -> Vec<f64> {
    let n = similarity.len();
    if n == 0 {
        return vec![];
    }

    let mut scores = vec![1.0 / n as f64; n];

    for _ in 0..iterations {
        let mut new_scores = vec![(1.0 - damping) / n as f64; n];
        for i in 0..n {
            let mut rank_sum = 0.0;
            for j in 0..n {
                if i != j && similarity[j][i] > 0.0 {
                    let out_weight: f64 = similarity[j].iter().sum();
                    if out_weight > 0.0 {
                        rank_sum += similarity[j][i] / out_weight * scores[j];
                    }
                }
            }
            new_scores[i] += damping * rank_sum;
        }
        scores = new_scores;
    }

    scores
}

/// Generate an extractive summary using TextRank.
///
/// # Arguments
/// * `text` - The input text to summarize
/// * `num_sentences` - Number of sentences to include in the summary
///
/// # Returns
/// A `SummaryResult` with the summary text and ranked sentences.
pub fn summarize(text: &str, num_sentences: usize) -> SummaryResult {
    let sentences = split_sentences(text);
    let original_count = sentences.len();

    if original_count == 0 {
        return SummaryResult {
            summary: String::new(),
            sentence_count: 0,
            original_count: 0,
            top_sentences: vec![],
        };
    }

    if original_count <= num_sentences {
        return SummaryResult {
            summary: sentences.join(". "),
            sentence_count: original_count,
            original_count,
            top_sentences: sentences,
        };
    }

    // Build bag-of-words vectors
    let bow_vectors: Vec<HashMap<String, f64>> = sentences
        .iter()
        .map(|s| {
            let tokens = tokenize(s);
            bow_vector(&tokens)
        })
        .collect();

    // Build similarity matrix
    let n = sentences.len();
    let mut similarity = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let sim = cosine_similarity(&bow_vectors[i], &bow_vectors[j]);
            similarity[i][j] = sim;
            similarity[j][i] = sim;
        }
    }

    // Run PageRank
    let scores = pagerank(&similarity, 0.85, 30);

    // Rank sentences by score
    let mut ranked: Vec<(usize, f64)> = scores.iter().enumerate().map(|(i, &s)| (i, s)).collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Select top-N sentences, preserving original order
    let mut selected: Vec<usize> = ranked.iter().take(num_sentences).map(|(i, _)| *i).collect();
    selected.sort();

    let top_sentences: Vec<String> = selected.iter().map(|&i| sentences[i].clone()).collect();
    let summary = top_sentences.join(". ");

    SummaryResult {
        summary,
        sentence_count: top_sentences.len(),
        original_count,
        top_sentences,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_short_text() {
        let text = "This is a short sentence. There are only two sentences here.";
        let result = summarize(text, 2);
        // Both sentences pass the >20 char filter
        assert_eq!(result.original_count, 2);
        // Since there are only 2 sentences and we asked for 2, both are included
        assert_eq!(result.sentence_count, 2);
    }

    #[test]
    fn test_summarize_long_text() {
        let text = "Artificial intelligence is transforming industries across the globe. \
                     Machine learning models can now process vast amounts of data in real time. \
                     The economic impact of AI is measured in trillions of dollars. \
                     However, concerns about bias and fairness remain significant. \
                     Researchers are working to make AI systems more transparent and accountable. \
                     The future of AI depends on responsible development practices. \
                     Companies are investing heavily in AI research and development. \
                     Governments are beginning to regulate AI applications.";

        let result = summarize(text, 3);
        assert_eq!(result.original_count, 8);
        assert_eq!(result.sentence_count, 3);
        assert!(!result.summary.is_empty());
        assert_eq!(result.top_sentences.len(), 3);
    }

    #[test]
    fn test_summarize_empty_text() {
        let result = summarize("", 3);
        assert_eq!(result.original_count, 0);
        assert!(result.summary.is_empty());
    }

    #[test]
    fn test_tokenize_filters_stop_words() {
        let tokens = tokenize("The quick brown fox jumps over the lazy dog");
        assert!(!tokens.contains(&"the".to_string()));
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let v = bow_vector(&["hello".to_string(), "world".to_string()]);
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 0.01);
    }
}
