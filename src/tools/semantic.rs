//! Semantic search using TF-IDF vectors and cosine similarity.
//!
//! Instead of requiring a heavy ONNX-based embedding model (fastembed-rs adds
//! ~100MB to the binary), this module uses TF-IDF (Term Frequency-Inverse
//! Document Frequency) vectors — a classic information-retrieval technique
//! that captures semantic similarity without any external model or API.
//!
//! TF-IDF works by:
//! 1. Building a vocabulary from all indexed documents
//! 2. Computing TF-IDF weights for each document (terms that are rare across
//!    the corpus but frequent in a document get high weights)
//! 3. Computing the query vector
//! 4. Ranking documents by cosine similarity to the query
//!
//! This gives "semantic-like" search (documents about "AI" will match queries
//! for "artificial intelligence" if they share contextual terms) without the
//! overhead of neural embeddings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    pub article_id: String,
    pub title: String,
    pub link: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchOutput {
    pub query: String,
    pub results: Vec<SemanticSearchResult>,
    pub count: usize,
}

/// A document in the search index.
#[derive(Debug, Clone)]
struct Document {
    id: String,
    title: String,
    link: String,
    text: String,
    tfidf: HashMap<String, f64>,
}

/// TF-IDF semantic search index.
pub struct SemanticIndex {
    documents: Vec<Document>,
    idf: HashMap<String, f64>,
    vocabulary: Vec<String>,
}

impl SemanticIndex {
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
            idf: HashMap::new(),
            vocabulary: Vec::new(),
        }
    }

    /// Add a document to the index. Triggers a recompute of IDF weights.
    pub fn add(&mut self, id: &str, title: &str, link: &str, text: &str) {
        let doc_text = format!("{} {}", title, text);
        self.documents.push(Document {
            id: id.to_string(),
            title: title.to_string(),
            link: link.to_string(),
            text: doc_text.clone(),
            tfidf: HashMap::new(), // will be computed in rebuild()
        });
        self.rebuild();
    }

    /// Add multiple documents and rebuild the index.
    pub fn add_batch(&mut self, docs: &[(String, String, String, String)]) {
        for (id, title, link, text) in docs {
            let doc_text = format!("{} {}", title, text);
            self.documents.push(Document {
                id: id.clone(),
                title: title.clone(),
                link: link.clone(),
                text: doc_text,
                tfidf: HashMap::new(),
            });
        }
        self.rebuild();
    }

    /// Rebuild IDF weights and TF-IDF vectors for all documents.
    fn rebuild(&mut self) {
        let n = self.documents.len();
        if n == 0 {
            return;
        }

        // Build vocabulary and document frequency
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        let mut doc_term_freqs: Vec<HashMap<String, f64>> = Vec::with_capacity(n);

        for doc in &self.documents {
            let tokens = tokenize(&doc.text);
            let mut tf: HashMap<String, f64> = HashMap::new();
            for token in &tokens {
                *tf.entry(token.clone()).or_insert(0.0) += 1.0;
            }
            // Normalize TF by document length
            let doc_len = tokens.len() as f64;
            if doc_len > 0.0 {
                for v in tf.values_mut() {
                    *v /= doc_len;
                }
            }
            // Update document frequency
            for term in tf.keys() {
                *doc_freq.entry(term.clone()).or_insert(0) += 1;
            }
            doc_term_freqs.push(tf);
        }

        // Compute IDF: log(N / df)
        self.idf = doc_freq
            .iter()
            .map(|(term, &df)| (term.clone(), ((n as f64) / (df as f64)).ln()))
            .collect();

        self.vocabulary = self.idf.keys().cloned().collect();

        // Compute TF-IDF vectors
        for (i, doc) in self.documents.iter_mut().enumerate() {
            doc.tfidf = doc_term_freqs[i]
                .iter()
                .map(|(term, &tf)| {
                    let idf = self.idf.get(term).copied().unwrap_or(0.0);
                    (term.clone(), tf * idf)
                })
                .collect();
            // Normalize by magnitude
            let magnitude = doc.tfidf.values().map(|v| v * v).sum::<f64>().sqrt();
            if magnitude > 0.0 {
                for v in doc.tfidf.values_mut() {
                    *v /= magnitude;
                }
            }
        }
    }

    /// Search the index for documents matching the query.
    /// Returns results sorted by cosine similarity (highest first).
    pub fn search(&self, query: &str, limit: usize) -> Vec<SemanticSearchResult> {
        if self.documents.is_empty() {
            return vec![];
        }

        // Compute query TF-IDF vector
        let query_tokens = tokenize(query);
        let mut query_tf: HashMap<String, f64> = HashMap::new();
        for token in &query_tokens {
            *query_tf.entry(token.clone()).or_insert(0.0) += 1.0;
        }
        let query_len = query_tokens.len() as f64;
        if query_len > 0.0 {
            for v in query_tf.values_mut() {
                *v /= query_len;
            }
        }

        let query_vec: HashMap<String, f64> = query_tf
            .iter()
            .map(|(term, &tf)| {
                let idf = self.idf.get(term).copied().unwrap_or(0.0);
                (term.clone(), tf * idf)
            })
            .collect();

        // Normalize query vector
        let query_mag = query_vec.values().map(|v| v * v).sum::<f64>().sqrt();
        let query_normalized: HashMap<String, f64> = if query_mag > 0.0 {
            query_vec.iter().map(|(k, &v)| (k.clone(), v / query_mag)).collect()
        } else {
            query_vec
        };

        // Compute cosine similarity for each document
        let mut results: Vec<SemanticSearchResult> = self
            .documents
            .iter()
            .map(|doc| {
                let score = cosine_similarity(&query_normalized, &doc.tfidf);
                SemanticSearchResult {
                    article_id: doc.id.clone(),
                    title: doc.title.clone(),
                    link: doc.link.clone(),
                    score,
                }
            })
            .filter(|r| r.score > 0.0)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    /// Get the number of documents in the index.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}

impl Default for SemanticIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Helper Functions ─────────────────────────────────────────

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect()
}

fn cosine_similarity(a: &HashMap<String, f64>, b: &HashMap<String, f64>) -> f64 {
    let mut dot = 0.0;
    let a_shorter = a.len() < b.len();
    if a_shorter {
        for (key, val_a) in a {
            if let Some(val_b) = b.get(key) {
                dot += val_a * val_b;
            }
        }
    } else {
        for (key, val_b) in b {
            if let Some(val_a) = a.get(key) {
                dot += val_a * val_b;
            }
        }
    }
    dot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_index_search() {
        let index = SemanticIndex::new();
        let results = index.search("test query", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_add_and_search() {
        let mut index = SemanticIndex::new();
        index.add("1", "AI breakthrough announced", "https://example.com/1", "OpenAI released GPT-5 with remarkable capabilities in artificial intelligence and machine learning");
        index.add("2", "Weather forecast for today", "https://example.com/2", "Sunny skies expected with mild temperatures across the region");
        index.add("3", "Machine learning advances", "https://example.com/3", "New models achieve state of the art performance on NLP benchmarks using transformer architectures");

        let results = index.search("artificial intelligence machine learning", 10);
        assert!(!results.is_empty());
        // Documents about AI/ML should rank higher than weather
        assert!(results[0].article_id == "1" || results[0].article_id == "3");
    }

    #[test]
    fn test_search_no_matches() {
        let mut index = SemanticIndex::new();
        index.add("1", "Hello world", "https://example.com/1", "Basic greeting text about programming");

        let results = index.search("xyzabc123 nonexistent", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_index_size() {
        let mut index = SemanticIndex::new();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());

        index.add("1", "Title 1", "url1", "text 1");
        assert_eq!(index.len(), 1);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_batch_add() {
        let mut index = SemanticIndex::new();
        let docs = vec![
            ("1".into(), "Title 1".into(), "url1".into(), "text one".into()),
            ("2".into(), "Title 2".into(), "url2".into(), "text two".into()),
            ("3".into(), "Title 3".into(), "url3".into(), "text three".into()),
        ];
        index.add_batch(&docs);
        assert_eq!(index.len(), 3);
    }
}
