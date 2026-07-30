//! Relevance scoring, caching, BM25 chunk scoring, dedup, and engine routing.
use crate::tools::types::*;
use crate::tools::nlp::tokenize;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};


// ─── Answer Synthesis (TextRank-based, no external LLM) ───────

/// Generate an answer from search results using extractive summarization.
/// Takes the top 5 highest-scored results, splits into sentences,
/// and returns the top-N most query-relevant sentences.
pub fn extractive_answer(results: &[WebSearchResult], query: &str) -> Option<String> {
    if results.is_empty() { return None; }

    // Collect text from top 5 results (up from 3 for better coverage)
    let query_words: Vec<String> = query.to_lowercase().split_whitespace()
        .filter(|w| w.len() > 1).map(String::from).collect();
    if query_words.is_empty() { return None; }

    let mut candidates: Vec<(f64, String)> = Vec::new();
    for result in results.iter().take(5) {
        let text = result.raw_content.as_deref().or(result.content.as_deref()).unwrap_or("");
        // Split into sentences
        for sentence in text.split(['.', '!', '?']) {
            let trimmed = sentence.trim();
            if trimmed.len() < 30 { continue; }
            // Score by query word overlap
            let lower = trimmed.to_lowercase();
            let score: f64 = query_words.iter().map(|w| if lower.contains(w.as_str()) { 1.0 } else { 0.0 }).sum();
            if score > 0.0 {
                candidates.push((score, trimmed.to_string()));
            }
        }
    }

    if candidates.is_empty() { return None; }

    // Sort by score, take top 5 (matches the 5 results we scan)
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<&str> = candidates.iter().take(5).map(|(_, s)| s.as_str()).collect();
    let answer = top.join(". ");
    if answer.is_empty() { None } else { Some(answer) }
}

// ─── In-Memory Search Cache ───────────────────────────────────

pub type CacheEntry = (WebSearchOutput, Instant);

pub static SEARCH_CACHE: LazyLock<std::sync::Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

// Cache TTL constants
pub const CACHE_TTL_NEWS: u64 = 300;          // 5 min for news
pub const CACHE_TTL_CODE: u64 = 43_200;       // 12 hours for code
pub const CACHE_TTL_GENERAL: u64 = 3_600;     // 1 hour for general
pub const CACHE_MAX_AGE: u64 = 86_400;         // 24h eviction ceiling
pub const CACHE_MAX_ENTRIES: usize = 100;

/// TTL for search results based on query type
pub fn cache_ttl(topic: &str) -> Duration {
    match topic {
        "news" => Duration::from_secs(CACHE_TTL_NEWS),
        "code" => Duration::from_secs(CACHE_TTL_CODE),
        _ => Duration::from_secs(CACHE_TTL_GENERAL),
    }
}

/// Check cache for a matching query. Returns None if miss or expired.
pub fn cache_get(query: &str, topic: &str, max_results: usize, content_length: &str, include_highlights: bool, include_answer: bool) -> Option<WebSearchOutput> {
    let cache_key = format!("{}:{}:{}:{}:{}:{}", topic, query.to_lowercase(), max_results, content_length, include_highlights, include_answer);
    let cache = SEARCH_CACHE.lock().ok()?;
    if let Some((output, inserted_at)) = cache.get(&cache_key) {
        if inserted_at.elapsed() < cache_ttl(topic) {
            return Some(output.clone());
        }
    }
    None
}

/// Store search results in cache.
pub fn cache_set(query: &str, topic: &str, max_results: usize, content_length: &str, include_highlights: bool, include_answer: bool, output: &WebSearchOutput) {
    let cache_key = format!("{}:{}:{}:{}:{}:{}", topic, query.to_lowercase(), max_results, content_length, include_highlights, include_answer);
    if let Ok(mut cache) = SEARCH_CACHE.lock() {
        // Evict expired entries when cache exceeds threshold
        if cache.len() > CACHE_MAX_ENTRIES {
            let max_ttl = Duration::from_secs(CACHE_MAX_AGE);
            cache.retain(|_, (_, inserted_at)| inserted_at.elapsed() < max_ttl);
        }
        cache.insert(cache_key, (output.clone(), Instant::now()));
    }
}

/// Extract domain from a URL string
pub fn extract_domain(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
}

/// Extract internal links from a parsed HTML document
pub fn extract_internal_links(
    doc: &scraper::Html,
    sel: &scraper::Selector,
    base_url: &url::Url,
    base_host: &str,
) -> Vec<String> {
    doc.select(sel)
        .filter_map(|el| el.attr("href"))
        .filter_map(|href| {
            url::Url::parse(href)
                .ok()
                .or_else(|| base_url.join(href).ok())
        })
        .map(|u| u.to_string())
        .filter(|url_str| {
            url::Url::parse(url_str)
                .ok()
                .and_then(|u| u.host_str().map(|s| s.to_string()))
                .unwrap_or_default()
                == base_host
        })
        .collect()
}

/// Domain authority lookup for relevance scoring.
/// Higher scores = more authoritative sources.
pub fn domain_authority(domain: &str) -> f64 {
    let lower = domain.to_lowercase();
    static AUTHORITIES: &[(&str, f64)] = &[
        ("github.com", 0.95),
        ("stackoverflow.com", 0.90),
        ("arxiv.org", 0.85),
        ("wikipedia.org", 0.85),
        ("news.ycombinator.com", 0.85),
        ("developer.mozilla.org", 0.90),
        ("docs.python.org", 0.85),
        ("docs.rs", 0.85),
        ("crates.io", 0.80),
        ("reddit.com", 0.70),
        ("medium.com", 0.65),
        ("dev.to", 0.65),
        ("stackoverflow.blog", 0.75),
        ("arstechnica.com", 0.75),
        ("techcrunch.com", 0.75),
        ("theverge.com", 0.70),
        ("reuters.com", 0.85),
        ("apnews.com", 0.85),
        ("bbc.com", 0.80),
        ("nytimes.com", 0.80),
        ("nature.com", 0.85),
        ("sciencedirect.com", 0.80),
    ];
    for (pattern, score) in AUTHORITIES {
        if lower.contains(pattern) {
            return *score;
        }
    }
    0.5 // default authority for unknown domains
}

/// Compute keyword relevance score (0.0-1.0) between a query and text.
/// Uses term frequency weighting.
pub fn keyword_relevance(title: &str, content: &str, query: &str) -> f64 {
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().filter(|w| w.len() > 1).collect();
    if query_words.is_empty() {
        return 0.5;
    }

    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();
    let combined = format!("{} {}", title_lower, content_lower);

    let mut matched = 0;
    for word in &query_words {
        if title_lower.contains(word) {
            matched += 2; // title match counts double
        } else if combined.contains(word) {
            matched += 1;
        }
    }

    (matched as f64 / (query_words.len() as f64 * 2.0)).min(1.0)
}

/// Compute freshness score (0.0-1.0) from a date string.
/// Newer results get higher scores.
pub fn freshness_score(published_date: Option<&str>) -> f64 {
    let Some(date_str) = published_date else {
        return 0.5; // unknown date gets neutral score
    };

    // Try to parse ISO date or relative time
    if let Ok(dt) = chrono::NaiveDate::parse_from_str(
        date_str.split('T').next().unwrap_or(date_str),
        "%Y-%m-%d"
    ) {
        let today = chrono::Utc::now().date_naive();
        let days_old = (today - dt).num_days().max(0);
        match days_old {
            0..=1 => 1.0,
            2..=7 => 0.9,
            8..=30 => 0.7,
            31..=90 => 0.5,
            91..=365 => 0.3,
            _ => 0.1,
        }
    } else if date_str.contains("year") {
        0.2
    } else if date_str.contains("month") {
        0.4
    } else if date_str.contains("week") {
        0.6
    } else if date_str.contains("day") {
        0.8
    } else if date_str.contains("hour") || date_str.contains("minute") || date_str.contains("just now") {
        1.0
    } else {
        0.5
    }
}

/// Compute composite relevance score for a search result.
/// Weights: keyword 50%, freshness 20%, domain authority 30%.
pub fn compute_relevance_score(result: &WebSearchResult, query: &str) -> f64 {
    let keyword = keyword_relevance(
        &result.title,
        result.content.as_deref().unwrap_or(""),
        query,
    );
    let freshness = freshness_score(result.published_date.as_deref());
    let authority = result.domain.as_deref().map(domain_authority).unwrap_or(0.5);

    keyword * 0.5 + freshness * 0.2 + authority * 0.3
}

/// Compute per-result confidence score (0.0-1.0).
/// Combines relevance score with snippet quality and highlight density.
/// Higher confidence = more trustworthy and actionable for AI agents.
pub fn compute_confidence(result: &WebSearchResult, query: &str) -> f64 {
    let relevance = compute_relevance_score(result, query);

    // Snippet quality: longer content snippets with more query terms = higher confidence
    let snippet_len = result.content.as_ref().map(|c| c.len()).unwrap_or(0);
    let snippet_quality = match snippet_len {
        0..=50 => 0.1,   // no snippet or very short
        51..=200 => 0.4,  // DDG-style short snippet
        201..=500 => 0.7, // standard excerpt
        501..=2000 => 0.9, // full excerpt (deep mode)
        _ => 1.0,          // very detailed
    };

    // Highlight density: more highlights = more relevant content found
    let highlight_count = result.highlights.as_ref().map(|h| h.len()).unwrap_or(0);
    let highlight_bonus = (highlight_count as f64 / 5.0).min(0.15);

    // Source diversity bonus: having a known domain = more trustworthy
    let domain_bonus = if result.domain.is_some() { 0.05 } else { 0.0 };

    (relevance * 0.6 + snippet_quality * 0.3 + highlight_bonus + domain_bonus).min(1.0)
}

/// Compute overall answer confidence from multiple search results.
/// Higher when results agree (similar scores), are from diverse sources,
/// and have high individual confidence.
pub fn compute_answer_confidence(results: &[WebSearchResult], query: &str) -> f64 {
    if results.is_empty() {
        return 0.0;
    }

    // Average individual confidence
    let avg_confidence: f64 = results.iter()
        .map(|r| compute_confidence(r, query))
        .sum::<f64>() / results.len() as f64;

    // Source diversity: unique domains = more trustworthy
    let unique_domains: usize = results.iter()
        .filter_map(|r| r.domain.as_ref())
        .collect::<HashSet<_>>()
        .len();
    let diversity_bonus = (unique_domains as f64 / results.len() as f64).min(0.2);

    // Score agreement: if top 3 results have similar scores, higher confidence
    let top_scores: Vec<f64> = results.iter().take(3)
        .filter_map(|r| r.score)
        .collect();
    let agreement_bonus = if top_scores.len() >= 2 {
        let spread = top_scores.iter().fold((f64::MAX, f64::MIN), |(min, max), &s| {
            (min.min(s), max.max(s))
        });
        let range = spread.1 - spread.0;
        if range < 0.1 { 0.15 } // high agreement
        else if range < 0.3 { 0.08 } // moderate agreement
        else { 0.0 } // low agreement
    } else {
        0.0
    };

    (avg_confidence * 0.6 + diversity_bonus + agreement_bonus).min(1.0)
}

/// Extract key sentences from text that match the query (highlights).
/// Returns up to 5 sentences, scored by query term overlap.
pub fn extract_highlights(text: &str, query: &str, max_highlights: usize) -> Vec<String> {
    let query_words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 1)
        .map(|w| w.to_string())
        .collect();

    if query_words.is_empty() || text.is_empty() {
        return vec![];
    }

    // Split into sentences
    let sentences: Vec<String> = text
        .split(['.', '!', '?'])
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() > 30 && s.len() < 500)
        .collect();

    // Score each sentence by query word overlap
    let mut scored: Vec<(f64, String)> = sentences
        .into_iter()
        .map(|s| {
            let lower = s.to_lowercase();
            let score: f64 = query_words
                .iter()
                .map(|w| if lower.contains(w.as_str()) { 1.0 } else { 0.0 })
                .sum();
            (score, s)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(max_highlights).map(|(_, s)| s).collect()
}

/// Truncate content to the specified length mode.
/// Uses char-counting to avoid panics on non-ASCII text.
pub fn truncate_content(content: Option<&str>, mode: &str) -> Option<String> {
    let text = content?;
    let max = match mode {
        "minimal" => 150,
        "full" => 2500,
        _ => 600, // standard
    };
    if text.chars().count() <= max {
        Some(text.to_string())
    } else {
        // Truncate to max chars, then trim to last word boundary
        let truncated: String = text.chars().take(max).collect();
        match truncated.rfind(' ') {
            Some(byte_pos) => {
                Some(format!("{}...", &truncated[..byte_pos]))
            }
            _ => Some(format!("{}...", truncated)),
        }
    }
}

// ─── BM25 / Cosine TF-IDF Chunk Scoring (CRW-inspired) ────────

// Reuse consolidated tokenizer from nlp module

/// Split text into paragraphs for BM25 chunk scoring.
/// Splits on double-newline boundaries, collapses internal single newlines,
/// and filters out fragments under 40 characters.
pub fn split_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|s| s.replace('\n', " "))
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() > 40)
        .collect()
}

/// BM25 chunk scoring constants.
pub const BM25_K1: f64 = 1.2;
pub const BM25_B: f64 = 0.75;
pub const BM25_MIN_THRESHOLD: f64 = 0.01;

/// A chunk with its relevance score and original index.
pub(crate) struct ScoredChunk {
    pub content: String,
    pub score: f64,
    pub index: usize,
}

/// Score and rank chunks by BM25 relevance to a query.
/// Standard BM25 algorithm (Robertson et al.).
pub(crate) fn bm25_score_chunks(chunks: &[String], query: &str, top_k: usize) -> Vec<ScoredChunk> {
    if chunks.is_empty() || query.trim().is_empty() {
        return chunks.iter().enumerate().map(|(i, c)| ScoredChunk {
            content: c.clone(), score: 0.0, index: i,
        }).collect();
    }

    let query_terms = tokenize(query, 2, false);
    let tokenized: Vec<Vec<String>> = chunks.iter().map(|c| tokenize(c, 2, false)).collect();
    let n = chunks.len() as f64;
    let avgdl = (tokenized.iter().map(|t| t.len()).sum::<usize>() as f64 / n).max(1.0);

    // Document frequency: how many chunks contain each term
    let mut df: HashMap<&str, usize> = HashMap::new();
    for doc in &tokenized {
        let mut seen: HashMap<&str, bool> = HashMap::new();
        for term in doc {
            if seen.insert(term.as_str(), true).is_none() {
                *df.entry(term.as_str()).or_insert(0) += 1;
            }
        }
    }

    let mut scored: Vec<(usize, f64)> = tokenized.iter().enumerate().map(|(i, doc)| {
        let dl = doc.len() as f64;
        let mut tf_map: HashMap<&str, usize> = HashMap::new();
        for term in doc { *tf_map.entry(term.as_str()).or_insert(0) += 1; }

        let score = query_terms.iter().map(|term| {
            let tf = *tf_map.get(term.as_str()).unwrap_or(&0) as f64;
            let df_t = *df.get(term.as_str()).unwrap_or(&0) as f64;
            let idf = ((n - df_t + 0.5) / (df_t + 0.5) + 1.0).ln();
            let tf_norm = (tf * (BM25_K1 + 1.0)) / (tf + BM25_K1 * (1.0 - BM25_B + BM25_B * dl / avgdl));
            idf * tf_norm
        }).sum::<f64>();

        (i, score)
    }).collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k.max(1).min(chunks.len()));
    scored.into_iter().map(|(i, score)| ScoredChunk {
        content: chunks[i].clone(), score, index: i,
    }).collect()
}

/// Jaccard similarity between two word sets (0.0-1.0).
pub fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<String> = a.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 1)
        .collect();
    let words_b: HashSet<String> = b.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 1)
        .collect();
    
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    
    let intersection = words_a.intersection(&words_b).count() as f64;
    let union = words_a.union(&words_b).count() as f64;
    
    if union == 0.0 { 0.0 } else { intersection / union }
}

/// Cross-source semantic dedup: remove results with >80% title similarity.
/// Keeps the version with higher relevance score.
pub fn semantic_dedup(results: &mut Vec<WebSearchResult>) {
    let threshold = 0.8;
    let mut to_remove = HashSet::new();
    
    for i in 0..results.len() {
        if to_remove.contains(&i) { continue; }
        for j in (i + 1)..results.len() {
            if to_remove.contains(&j) { continue; }
            
            let sim = jaccard_similarity(&results[i].title, &results[j].title);
            if sim > threshold {
                // Keep the one with higher score (tiebreak: keep earlier result)
                let score_i = results[i].score.unwrap_or(0.0);
                let score_j = results[j].score.unwrap_or(0.0);
                if score_i > score_j {
                    to_remove.insert(j);
                } else {
                    // Equal scores or j is higher: remove i (keep j, which is later)
                    to_remove.insert(i);
                    break; // i is removed, no need to compare further
                }
            }
        }
    }
    
    if !to_remove.is_empty() {
        let mut idx = 0;
        results.retain(|_| {
            let keep = !to_remove.contains(&idx);
            idx += 1;
            keep
        });
    }
}

/// Domain-level dedup: keep highest-scoring result per registrable domain.
pub fn domain_dedup(results: &mut Vec<WebSearchResult>) {
    // Common multi-part TLDs that should be treated as a single suffix
    static MULTI_PART_TLDS: &[&str] = &[
        "co.uk", "com.au", "co.nz", "co.za", "co.in", "co.jp",
        "com.br", "com.cn", "com.mx", "com.sg", "com.tw",
        "or.jp", "ne.jp", "net.au", "org.au",
    ];

    // Extract registrable domain (e.g., "bbc.co.uk" from "www.bbc.co.uk")
    fn registrable_domain(domain: &str) -> String {
        let lower = domain.to_lowercase();
        // Check for known multi-part TLDs
        for tld in MULTI_PART_TLDS {
            if let Some(pos) = lower.rfind(tld) {
                let before = &lower[..pos];
                if let Some(dot) = before.rfind('.') {
                    return format!("{}{}", &before[dot + 1..], tld);
                }
            }
        }
        // Default: last two labels
        let parts: Vec<&str> = lower.split('.').collect();
        if parts.len() <= 2 {
            lower
        } else {
            format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
        }
    }

    let mut best_by_domain: HashMap<String, usize> = HashMap::new();
    let mut to_remove = HashSet::new();

    for (i, result) in results.iter().enumerate() {
        if let Some(ref domain) = result.domain {
            let reg = registrable_domain(domain);
            if let Some(&prev_idx) = best_by_domain.get(&reg) {
                // Keep the one with higher score
                let prev_score = results[prev_idx].score.unwrap_or(0.0);
                let cur_score = result.score.unwrap_or(0.0);
                if cur_score > prev_score {
                    to_remove.insert(prev_idx);
                    best_by_domain.insert(reg, i);
                } else {
                    to_remove.insert(i);
                }
            } else {
                best_by_domain.insert(reg, i);
            }
        }
    }

    if !to_remove.is_empty() {
        let mut idx = 0;
        results.retain(|_| {
            let keep = !to_remove.contains(&idx);
            idx += 1;
            keep
        });
    }
}

/// Smart topic-based engine routing.
/// Automatically selects optimal engines based on query intent.
pub fn route_engines(topic: &str, query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();

    // Detect topic from query keywords if not explicit
    let effective_topic = if topic != "general" {
        topic
    } else if query_lower.contains("rust") || query_lower.contains("python") || query_lower.contains("javascript")
        || query_lower.contains("code") || query_lower.contains("api") || query_lower.contains("library")
        || query_lower.contains("function") || query_lower.contains("error") || query_lower.contains("bug")
        || query_lower.contains("compiler") || query_lower.contains("package") || query_lower.contains("crate")
    {
        "code"
    } else if query_lower.contains("news") || query_lower.contains("breaking") || query_lower.contains("today")
        || query_lower.contains("yesterday") || query_lower.contains("latest") || query_lower.contains("recent")
    {
        "news"
    } else if query_lower.contains("research") || query_lower.contains("paper") || query_lower.contains("study")
        || query_lower.contains("journal") || query_lower.contains("clinical") || query_lower.contains("trial")
    {
        "academic"
    } else {
        "general"
    };

    // Detect video intent from query keywords
    let has_video_intent = query_lower.contains("video")
        || query_lower.contains("tutorial")
        || query_lower.contains("watch")
        || query_lower.contains("how to")
        || query_lower.contains("demo");

    let mut engines = match effective_topic {
        "code" => vec![
            "github".to_string(),
            "stackoverflow".to_string(),
            "hackernews".to_string(),
            "duckduckgo".to_string(),
        ],
        "news" => vec![
            "duckduckgo".to_string(),
            "hackernews".to_string(),
        ],
        "academic" => vec![
            "wikipedia".to_string(),
            "github".to_string(),
            "duckduckgo".to_string(),
        ],
        _ => vec![ // general
            "duckduckgo".to_string(),
            "wikipedia".to_string(),
            "hackernews".to_string(),
        ],
    };

    // Add YouTube when video intent is detected
    if has_video_intent {
        engines.push("youtube".to_string());
    }

    engines
}

