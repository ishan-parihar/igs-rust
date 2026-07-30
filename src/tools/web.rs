use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types::*;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

// ─── Answer Synthesis (TextRank-based, no external LLM) ───────

/// Generate an answer from search results using extractive summarization.
/// Takes the top 3 highest-scored results, splits into sentences,
/// and returns the top-N most query-relevant sentences.
fn extractive_answer(results: &[WebSearchResult], query: &str) -> Option<String> {
    if results.is_empty() { return None; }

    // Collect text from top 3 results
    let query_words: Vec<String> = query.to_lowercase().split_whitespace()
        .filter(|w| w.len() > 1).map(String::from).collect();
    if query_words.is_empty() { return None; }

    let mut candidates: Vec<(f64, String)> = Vec::new();
    for result in results.iter().take(3) {
        let text = result.content.as_deref().unwrap_or("");
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

    // Sort by score, take top 3
    candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<&str> = candidates.iter().take(3).map(|(_, s)| s.as_str()).collect();
    let answer = top.join(". ");
    if answer.is_empty() { None } else { Some(answer) }
}

// ─── In-Memory Search Cache ───────────────────────────────────

type CacheEntry = (WebSearchOutput, Instant);

static SEARCH_CACHE: LazyLock<std::sync::Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

// Cache TTL constants
const CACHE_TTL_NEWS: u64 = 300;          // 5 min for news
const CACHE_TTL_CODE: u64 = 43_200;       // 12 hours for code
const CACHE_TTL_GENERAL: u64 = 3_600;     // 1 hour for general
const CACHE_MAX_AGE: u64 = 86_400;         // 24h eviction ceiling
const CACHE_MAX_ENTRIES: usize = 100;

/// TTL for search results based on query type
fn cache_ttl(topic: &str) -> Duration {
    match topic {
        "news" => Duration::from_secs(CACHE_TTL_NEWS),
        "code" => Duration::from_secs(CACHE_TTL_CODE),
        _ => Duration::from_secs(CACHE_TTL_GENERAL),
    }
}

/// Check cache for a matching query. Returns None if miss or expired.
fn cache_get(query: &str, topic: &str, max_results: usize, content_length: &str, include_highlights: bool, include_answer: bool) -> Option<WebSearchOutput> {
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
fn cache_set(query: &str, topic: &str, max_results: usize, content_length: &str, include_highlights: bool, include_answer: bool, output: &WebSearchOutput) {
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
fn extract_domain(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
}

/// Extract internal links from a parsed HTML document
fn extract_internal_links(
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
fn domain_authority(domain: &str) -> f64 {
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
fn keyword_relevance(title: &str, content: &str, query: &str) -> f64 {
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
fn freshness_score(published_date: Option<&str>) -> f64 {
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
fn compute_relevance_score(result: &WebSearchResult, query: &str) -> f64 {
    let keyword = keyword_relevance(
        &result.title,
        result.content.as_deref().unwrap_or(""),
        query,
    );
    let freshness = freshness_score(result.published_date.as_deref());
    let authority = result.domain.as_deref().map(domain_authority).unwrap_or(0.5);

    keyword * 0.5 + freshness * 0.2 + authority * 0.3
}

/// Extract key sentences from text that match the query (highlights).
/// Returns up to 5 sentences, scored by query term overlap.
fn extract_highlights(text: &str, query: &str, max_highlights: usize) -> Vec<String> {
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
fn truncate_content(content: Option<&str>, mode: &str) -> Option<String> {
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

/// Tokenize text into lowercase alphanumeric terms (len > 1).
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .map(|t| t.to_string())
        .collect()
}

/// BM25 chunk scoring constants.
const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;
const BM25_MIN_THRESHOLD: f64 = 0.01;

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

    let query_terms = tokenize(query);
    let tokenized: Vec<Vec<String>> = chunks.iter().map(|c| tokenize(c)).collect();
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
fn jaccard_similarity(a: &str, b: &str) -> f64 {
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
fn semantic_dedup(results: &mut Vec<WebSearchResult>) {
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
fn domain_dedup(results: &mut Vec<WebSearchResult>) {
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
fn route_engines(topic: &str, query: &str) -> Vec<String> {
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

/// Search the web using multiple engines in parallel.
/// Engines: duckduckgo (Obscura, free), wikipedia (REST API, free),
///          github (REST API, free), hackernews (Algolia API, free), stackoverflow (StackExchange API, free).
/// Zero API keys required. Supports "fast" mode (snippets only) and "deep" mode (scrape result pages for 500-2000 char excerpts).
pub async fn web_search(input: WebSearchInput) -> Result<WebSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let max_results: usize = input.max_results.unwrap_or(10) as usize;
    let depth = input.depth.as_deref().unwrap_or("fast");
    let topic = input.topic.as_deref().unwrap_or("general");
    let content_length = input.content_length.as_deref().unwrap_or("standard");
    let include_highlights = input.include_highlights.unwrap_or(false);
    let is_deep = depth == "deep";

    // Check cache first (skip for deep mode or explicit engines)
    if !is_deep && input.engines.is_none() {
        if let Some(cached) = cache_get(&input.query, topic, max_results, content_length, include_highlights, input.include_answer.unwrap_or(false)) {
            return Ok(cached);
        }
    }

    // Determine which engines to use
    let engines = input.engines.clone().unwrap_or_else(|| route_engines(topic, &input.query));

    let start = Instant::now();

    // Launch all engine queries in parallel
    let mut handles = Vec::new();

    for engine in &engines {
        match engine.as_str() {
            "duckduckgo" => {
                let q = input.query.clone();
                let obs_settings = settings.browser.obscura.clone();
                let include_answer = input.include_answer.unwrap_or(false);
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                handles.push(tokio::spawn(async move {
                    search_duckduckgo(&q, max_results * 2, include_answer, &obs_settings, &http_clone).await
                }));
            }

            "wikipedia" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                handles.push(tokio::spawn(async move {
                    search_wikipedia(&q, (max_results / 2).max(3), &http_clone).await
                }));
            }
            "github" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                let topic_clone = topic.to_string();
                handles.push(tokio::spawn(async move {
                    search_github(&q, max_results, &http_clone, &topic_clone).await
                }));
            }
            "hackernews" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                let time_range_clone = input.time_range.clone().unwrap_or_default();
                handles.push(tokio::spawn(async move {
                    search_hackernews(&q, max_results, &http_clone, &time_range_clone).await
                }));
            }
            "stackoverflow" => {
                let q = input.query.clone();
                let http_clone = HttpClient::new(&settings.http, &cache_dir);
                handles.push(tokio::spawn(async move {
                    search_stackoverflow(&q, max_results, &http_clone).await
                }));
            }
            "youtube" => {
                let q = input.query.clone();
                handles.push(tokio::spawn(async move {
                    search_youtube(&q, max_results).await
                }));
            }
            _ => {} // skip unknown engines
        }
    }

    // Collect results from all engines
    let mut all_results: Vec<WebSearchResult> = Vec::new();
    let mut engines_used: Vec<String> = Vec::new();
    let mut answer: Option<String> = None;

    for handle in handles {
        match handle.await {
            Ok(Ok((engine_name, mut results, engine_answer))) => {
                if !results.is_empty() {
                    engines_used.push(engine_name);
                }
                all_results.append(&mut results);
                if answer.is_none() && engine_answer.is_some() {
                    answer = engine_answer;
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Search engine error: {}", e);
            }
            Err(e) => {
                tracing::warn!("Search engine task panicked: {}", e);
            }
        }
    }

    // Dedup by URL (keep first occurrence, which has priority ordering)
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut deduped: Vec<WebSearchResult> = Vec::new();
    for result in all_results {
        if seen_urls.insert(result.url.clone()) {
            deduped.push(result);
        }
    }

    // Apply domain filters
    if let Some(ref include) = input.include_domains {
        let include_lower: Vec<String> = include.iter().map(|d| d.to_lowercase()).collect();
        deduped.retain(|r| {
            let domain = r.domain.as_deref().unwrap_or("").to_lowercase();
            include_lower.iter().any(|d| domain.contains(d.as_str()))
        });
    }
    if let Some(ref exclude) = input.exclude_domains {
        let exclude_lower: Vec<String> = exclude.iter().map(|d| d.to_lowercase()).collect();
        deduped.retain(|r| {
            let domain = r.domain.as_deref().unwrap_or("").to_lowercase();
            !exclude_lower.iter().any(|d| domain.contains(d.as_str()))
        });
    }

    // Compute relevance scores for all results
    for result in &mut deduped {
        result.score = Some(compute_relevance_score(result, &input.query));
    }

    // Semantic dedup: remove results with >80% title similarity
    semantic_dedup(&mut deduped);

    // Domain-level dedup: keep highest-scoring result per registrable domain
    domain_dedup(&mut deduped);

    // Sort by score (highest first)
    deduped.sort_by(|a, b| {
        b.score.unwrap_or(0.0)
            .partial_cmp(&a.score.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deep mode: scrape each result page for semantic excerpts
    if is_deep && !deduped.is_empty() {
        let obs_settings = settings.browser.obscura.clone();
        if obs_settings.enabled {
            let obscura = crate::obscura::ObscuraManager::new(&obs_settings);
            for result in &mut deduped {
                if let Ok(html) = obscura
                    .fetch_with_all_options(&result.url, "html", false, "load", false, None)
                    .await
                {
                    let excerpt = extract_semantic_excerpt(&html, &result.title, 1500);
                    if !excerpt.is_empty() {
                        result.raw_content = Some(html_to_markdown_rs::convert(&html, None)
                            .ok()
                            .and_then(|r| r.content)
                            .unwrap_or_default());
                        // Upgrade content if the excerpt is better than the snippet
                        if excerpt.len() > result.content.as_deref().unwrap_or("").len() {
                            result.content = Some(excerpt);
                        }
                    }
                }
            }
        }
    }

    // Apply content depth control (skip in deep mode — deep already provides full excerpts)
    if !is_deep {
        for result in &mut deduped {
            result.content = truncate_content(result.content.as_deref(), content_length);
        }
    }

    // Extract highlights if requested (only from substantial text)
    if include_highlights {
        for result in &mut deduped {
            let text = result
                .raw_content
                .as_deref()
                .or(result.content.as_deref())
                .unwrap_or("");
            // Only extract highlights from text with enough content (>200 chars)
            if text.len() > 200 {
                let hl = extract_highlights(text, &input.query, 5);
                if !hl.is_empty() {
                    result.highlights = Some(hl);
                }
            }
        }
    }

    // Truncate to max_results
    deduped.truncate(max_results);

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let count = deduped.len();

    // Answer synthesis: generate from top results if not already provided by engines
    if answer.is_none() && input.include_answer.unwrap_or(false) {
        answer = extractive_answer(&deduped, &input.query);
    }

    // Compute answer confidence: based on engine count and result diversity
    let confidence = if answer.is_some() {
        let engine_count = engines_used.len() as f64;
        let domain_count = deduped.iter()
            .filter_map(|r| r.domain.as_ref())
            .collect::<HashSet<_>>()
            .len() as f64;
        let base = 0.5 + (engine_count * 0.1).min(0.3) + (domain_count * 0.05).min(0.2);
        Some(base.min(1.0))
    } else {
        None
    };

    let output = WebSearchOutput {
        count,
        results: deduped,
        answer,
        confidence,
        meta: WebSearchMeta {
            provider: engines_used.join("+"),
            query: input.query.clone(),
            engines_used,
            response_time_ms: elapsed_ms,
            total_results: count,
            scored: Some(true),
        },
    };

    // Cache the results (skip for deep mode or explicit engines)
    if !is_deep && input.engines.is_none() {
        cache_set(&input.query, topic, max_results, content_length, include_highlights, input.include_answer.unwrap_or(false), &output);
    }

    Ok(output)
}

// ─── Readability Extractor (CRW-inspired) ─────────────────────

/// Compute text-to-HTML ratio as a content density signal.
/// Returns a value in [0, 1]: higher = more text relative to markup.
/// Ported from CRW's text_density function.
fn text_density(html: &str) -> f64 {
    let doc = scraper::Html::parse_fragment(html);
    let text_len: usize = doc.root_element().text().map(|t| t.len()).sum();
    if html.is_empty() {
        return 0.0;
    }
    text_len as f64 / html.len() as f64
}

/// Scored selectors for main content extraction, ordered by priority.
/// Copied from CRW's readability.rs selector list.
const SCORED_SELECTORS: &[&str] = &[
    // Priority: semantic HTML first
    "article", "main", "[role='main']",
    // Content containers
    ".post-content", ".article-body", ".entry-content",
    ".article-content", ".post-body", ".story-body",
    ".content-body", "#main-content", "#article",
    "#content", ".content", ".main",
    "[itemprop='articleBody']", "[itemprop='text']",
    // MDN
    ".main-page-content",
    // StackOverflow
    ".js-post-body", ".s-prose", "#question",
    // Generic
    ".page-content", "#page-content", "[role='article']",
    // Wikipedia / MediaWiki
    ".mw-parser-output", "#mw-content-text",
    "#bodyContent", ".mw-body-content",
];

/// Inner selectors for drill-down when a priority selector is too broad (>90% of body).
const DRILL_DOWN_SELECTORS: &[&str] = &[
    ".main-page-content", ".article-content", ".post-content",
    ".entry-content", ".content-body", ".article-body",
    "[itemprop='articleBody']", "[itemprop='text']",
    ".mw-parser-output", "#mw-content-text", "#content",
    ".content", "article",
];

/// When a priority selector is "too broad" (>90% of body), drill down into it
/// to find a narrower content element. Ported from CRW's find_content_within.
fn find_content_within(parent_el: &scraper::ElementRef, parent_len: usize) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    for sel_str in DRILL_DOWN_SELECTORS {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            for el in parent_el.select(&sel) {
                let content = el.html();
                if content.len() < 200 {
                    continue;
                }
                // Skip if still too broad relative to parent
                if content.len() as f64 / parent_len as f64 > 0.85 {
                    continue;
                }
                let score = text_density(&content) * (content.len() as f64).ln();
                if best.as_ref().is_none_or(|(_, s)| score > *s) {
                    best = Some((content, score));
                }
            }
        }
    }
    best.map(|(c, _)| c)
}

/// Extract the main content from HTML using text-density scoring.
/// Ported from CRW's extract_main_content with drill-down logic.
/// Falls back to body if no scored candidate is found.
fn extract_main_content(html: &str) -> String {
    let document = scraper::Html::parse_document(html);

    // Compute body length once for ratio checks
    let body_len = scraper::Selector::parse("body")
        .ok()
        .and_then(|sel| document.select(&sel).next())
        .map(|b| b.html().len())
        .unwrap_or(html.len());

    // Score all candidate selectors by text density and pick the best
    let mut best: Option<(String, f64)> = None;
    for sel_str in SCORED_SELECTORS {
        if let Ok(sel) = scraper::Selector::parse(sel_str) {
            for el in document.select(&sel) {
                let content = el.html();
                if content.len() < 100 {
                    continue;
                }
                // Skip selectors that wrap nearly the entire body
                if body_len > 0 && content.len() as f64 / body_len as f64 > 0.9 {
                    if let Some(narrowed) = find_content_within(&el, content.len()) {
                        return narrowed;
                    }
                    continue;
                }
                let score = text_density(&content) * (content.len() as f64).ln();
                if best.as_ref().is_none_or(|(_, s)| score > *s) {
                    best = Some((content, score));
                }
            }
        }
    }

    if let Some((content, _)) = best {
        return content;
    }

    // Last resort: return full body
    if let Ok(sel) = scraper::Selector::parse("body") {
        if let Some(body) = document.select(&sel).next() {
            return body.inner_html();
        }
    }

    html.to_string()
}

/// Extract a semantic excerpt from a page.
/// Uses CRW-inspired text-density scoring with drill-down logic.
fn extract_semantic_excerpt(html: &str, _title: &str, max_chars: usize) -> String {
    let main_html = extract_main_content(html);
    let doc = scraper::Html::parse_fragment(&main_html);
    let text: String = doc.root_element().text().collect::<Vec<_>>().join(" ");
    let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned.chars().take(max_chars).collect()
}

/// Extract the real URL from a DuckDuckGo redirect link.
/// DuckDuckGo wraps all result links in /l/?uddg=<encoded_url>&rut=...
/// Works for both relative (/l/?uddg=...) and absolute URLs.
fn extract_ddg_redirect_url(href: &str) -> Option<String> {
    let pos = href.find("uddg=")?;
    let encoded = &href[pos + 5..];
    let end = encoded.find('&').unwrap_or(encoded.len());
    url::form_urlencoded::parse(format!("x={}", &encoded[..end]).as_bytes())
        .find(|(k, _)| k == "x")
        .map(|(_, v)| v.to_string())
}

// ─── YouTube Search Engine (via yt-dlp) ──────────────────────

/// Search YouTube via yt-dlp (already available on the system).
/// Returns (engine_name, results, answer). Key-free.
async fn search_youtube(
    query: &str,
    max_results: usize,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let limit = max_results.min(10);
    let search_term = format!("ytsearch{}:{}", limit, query);

    // Fail gracefully — don't break web.search if yt-dlp is missing
    let output = match tokio::process::Command::new("yt-dlp")
        .args([
            &search_term,
            "--flat-playlist",
            "--print",
            "%(id)s|||%(title)s|||%(channel)s|||%(duration_string)s",
        ])
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return Ok(("youtube".to_string(), vec![], None)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("ERROR") {
            tracing::warn!("yt-dlp error: {}", stderr.trim());
            return Ok(("youtube".to_string(), vec![], None));
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.splitn(4, "|||").collect();
        if parts.len() < 3 { continue; }

        let video_id = parts[0];
        let title = parts[1];
        let channel = parts[2];
        let duration = if parts.len() > 3 && !parts[3].is_empty() {
            Some(parts[3].to_string())
        } else {
            None
        };

        let url = format!("https://www.youtube.com/watch?v={}", video_id);
        let content = format!("📺 {} | {}", channel, duration.unwrap_or_default());

        results.push(WebSearchResult {
            title: title.to_string(),
            url,
            content: Some(content),
            score: Some(0.7),
            highlights: None,
            raw_content: None,
            source: Some("youtube".to_string()),
            domain: Some("youtube.com".to_string()),
            published_date: None,
            favicon: None,
        });
    }

    Ok(("youtube".to_string(), results, None))
}

/// Search for images via Wikimedia Commons REST API (key-free).
/// Uses the Wikimedia Commons API to find freely licensed images.
/// No API key required — Wikimedia is completely open.
pub async fn web_image_search(input: WebImageSearchInput) -> Result<WebImageSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;

    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    let max_results = input.max_results.unwrap_or(10).min(30) as usize;
    let query_encoded = url::form_urlencoded::byte_serialize(input.query.as_bytes()).collect::<String>();

    let start = std::time::Instant::now();

    // Wikimedia Commons API — search for images in namespace 6 (File)
    let search_url = format!(
        "https://commons.wikimedia.org/w/api.php?action=query&generator=search&gsrsearch=file:{}&gsrnamespace=6&prop=imageinfo&iiprop=url|extmetadata|size&iiurlwidth=800&format=json",
        query_encoded
    );

    let mut results = Vec::new();
    let mut total_available = 0;

    match http.fetch(&search_url, None, "bypass").await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                // Get total count if available, fallback to pages count
                if let Some(query_info) = json["query"]["searchinfo"].as_object() {
                    total_available = query_info["totalhits"].as_u64().unwrap_or(0) as usize;
                }

                if let Some(pages) = json["query"]["pages"].as_object() {
                    // Only count image MIME types (exclude PDFs, SVGs with no raster, etc.)

                    for (_, page) in pages {
                        if results.len() >= max_results {
                            break;
                        }

                        let title = page["title"].as_str().unwrap_or("Untitled").to_string();

                        // Skip non-image files (PDFs, documents, etc.)
                        let title_lower = title.to_lowercase();
                        if title_lower.ends_with(".pdf")
                            || title_lower.ends_with(".svg")
                            || title_lower.ends_with(".mid")
                            || title_lower.ends_with(".midi")
                        {
                            continue;
                        }

                        // Extract image URL from imageinfo
                        if let Some(imageinfo) = page["imageinfo"].as_array() {
                            if let Some(info) = imageinfo.first() {
                                let url = info["url"].as_str().unwrap_or("").to_string();
                                let thumb_url = info["thumburl"].as_str().map(|s| s.to_string());
                                let width = info["width"].as_u64().map(|w| w as u32);
                                let height = info["height"].as_u64().map(|h| h as u32);

                                // Source page URL on Wikimedia Commons
                                let source_url = Some(format!(
                                    "https://commons.wikimedia.org/wiki/{}",
                                    title.replace(' ', "_")
                                ));

                                if !url.is_empty() {
                                    // Extract image description from Wikimedia extmetadata
                                let description = info["extmetadata"]
                                    .as_object()
                                    .and_then(|em| em.get("ImageDescription"))
                                    .and_then(|desc| desc.get("value"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| {
                                        // Strip HTML tags using scraper for clean text extraction
                                        let doc = scraper::Html::parse_fragment(s);
                                        doc.root_element().text().collect::<Vec<_>>().join(" ")
                                    })
                                    .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                                    .filter(|s| !s.is_empty());

                                results.push(WebImageResult {
                                    title,
                                    url,
                                    thumbnail: thumb_url,
                                    source_url,
                                    width,
                                    height,
                                    source: Some("wikimedia_commons".to_string()),
                                    description,
                                });
                                }
                            }
                        }
                    }

                }

                // Fallback: if total_available wasn't set, use results count
                // Runs outside pages block to handle edge case where API
                // returns searchinfo but no pages object
                if total_available == 0 {
                    total_available = results.len();
                }
            }
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => {
            tracing::warn!("Wikimedia Commons API: unexpected cached response in bypass mode");
        }
        Err(e) => {
            tracing::warn!("Wikimedia Commons API error: {}", e);
        }
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let count = results.len();

    Ok(WebImageSearchOutput {
        results,
        count,
        meta: WebSearchMeta {
            provider: "wikimedia_commons".to_string(),
            query: input.query.clone(),
            engines_used: vec!["wikimedia_commons".to_string()],
            response_time_ms: elapsed_ms,
            total_results: total_available.max(count),
            scored: Some(false),
        },
    })
}

// ─── DuckDuckGo Search Engine ─────────────────────────────────

/// Parse DuckDuckGo HTML search results page.
/// Domain filtering is handled by web_search after dedup.
fn parse_duckduckgo_html(html: &str, max_results: usize) -> Vec<WebSearchResult> {
    let doc = scraper::Html::parse_document(html);
    let mut results = Vec::new();

    let selectors = [".result__body", ".web-result", ".results_links", ".result"];

    for selector_str in &selectors {
        if let Ok(sel) = scraper::Selector::parse(selector_str) {
            for element in doc.select(&sel).take(max_results * 2) {
                if let Ok(link_sel) = scraper::Selector::parse("a.result__a") {
                    if let Some(link_el) = element.select(&link_sel).next() {
                        let title = link_el.text().collect::<String>().trim().to_string();
                        let raw_href = link_el.attr("href").unwrap_or("").to_string();
                        let url = extract_ddg_redirect_url(&raw_href).unwrap_or(raw_href);

                        let snippet = scraper::Selector::parse(".result__snippet")
                            .ok()
                            .and_then(|s| element.select(&s).next())
                            .map(|el| el.text().collect::<String>().trim().to_string());

                        let domain = scraper::Selector::parse(".result__url")
                            .ok()
                            .and_then(|s| element.select(&s).next())
                            .map(|el| el.text().collect::<String>().trim().to_string())
                            .or_else(|| extract_domain(&url));

                        if !url.is_empty() && !title.is_empty() {
                            results.push(WebSearchResult {
                                title,
                                url,
                                content: snippet,
                                score: None,
                                highlights: None,
                                raw_content: None,
                                source: Some("duckduckgo".to_string()),
                                domain,
                                published_date: None,
                                favicon: None,
                            });
                        }
                    }
                }
            }
            if !results.is_empty() { break; }
        }
    }
    results.truncate(max_results);
    results
}

/// Search DuckDuckGo via Obscura headless browser. Returns (engine_name, results, answer).
async fn search_duckduckgo(
    query: &str,
    max_results: usize,
    include_answer: bool,
    obs_settings: &crate::types::ObscuraSettings,
    http: &HttpClient,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    if !obs_settings.enabled {
        return Err("Obscura not enabled".into());
    }

    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let search_url = format!("https://html.duckduckgo.com/html/?q={}", query_encoded);

    let html = obscura
        .fetch_with_all_options(&search_url, "html", false, "load", false, None)
        .await
        .map_err(|e| format!("DDG search failed: {}", e))?;

    let results = parse_duckduckgo_html(&html, max_results);

    // Optionally fetch DDG Instant Answer API for a synthesized answer
    let answer = if include_answer {
        let ia_url = format!("https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1", query_encoded);
        match http.fetch(&ia_url, None, "bypass").await {
            Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
                serde_json::from_str::<serde_json::Value>(&resp.body_text)
                    .ok()
                    .and_then(|j| j["AbstractText"].as_str().map(|s| s.to_string()))
                    .filter(|s| !s.is_empty())
            }
            _ => None,
        }
    } else {
        None
    };

    Ok(("duckduckgo".to_string(), results, answer))
}




// ─── Wikipedia REST API Engine ────────────────────────────────

/// Search Wikipedia via their free REST API. Returns (engine_name, results, answer).
async fn search_wikipedia(
    query: &str,
    max_results: usize,
    http: &HttpClient,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let search_url = format!(
        "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
        query_encoded
    );

    // Try the summary endpoint first (direct article lookup)
    let mut results = Vec::new();
    let mut answer = None;

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&search_url, None, "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(title) = json["title"].as_str() {
                let url = json["content_urls"]["desktop"]["page"].as_str().unwrap_or("").to_string();
                let extract = json["extract"].as_str().unwrap_or("").to_string();
                let thumbnail = json["thumbnail"]["source"].as_str().map(|s| s.to_string());

                if !url.is_empty() {
                    answer = Some(extract.clone());
                    results.push(WebSearchResult {
                        title: title.to_string(),
                        url,
                        content: Some(extract),
                        score: Some(1.0),
                        highlights: None,
                        raw_content: None,
                        source: Some("wikipedia".to_string()),
                        domain: Some("wikipedia.org".to_string()),
                        published_date: None,
                        favicon: thumbnail,
                    });
                }
            }
        }
    }

    // Also do a search for related articles
    let search_api_url = format!(
        "https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit={}",
        query_encoded, max_results
    );

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&search_api_url, None, "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(search_results) = json["query"]["search"].as_array() {
                for r in search_results.iter().take(max_results) {
                    let title = r["title"].as_str().unwrap_or("").to_string();
                    let snippet = r["snippet"].as_str()
                        .unwrap_or("")
                        .replace("<span class=\"searchmatch\">", "")
                        .replace("</span>", "")
                        .to_string();
                    let url = format!("https://en.wikipedia.org/wiki/{}", title.replace(' ', "_"));

                    // Skip if we already have this URL from the summary endpoint
                    if results.iter().any(|existing| existing.url == url) { continue; }

                    results.push(WebSearchResult {
                        title,
                        url,
                        content: if snippet.is_empty() { None } else { Some(snippet) },
                        score: Some(0.8),
                        highlights: None,
                        raw_content: None,
                        source: Some("wikipedia".to_string()),
                        domain: Some("wikipedia.org".to_string()),
                        published_date: None,
                        favicon: None,
                    });
                }
            }
        }
    }

    Ok(("wikipedia".to_string(), results, answer))
}

// ─── GitHub Search API Engine ──────────────────────────────────

/// Search GitHub via their free REST API. Returns (engine_name, results, answer).
/// When topic="code", also searches for matching code files.
async fn search_github(
    query: &str,
    max_results: usize,
    http: &HttpClient,
    topic: &str,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let search_url = format!(
        "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page={}",
        query_encoded, max_results.min(10)
    );

    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert("Accept".to_string(), "application/vnd.github.v3+json".to_string());

    let mut results = Vec::new();

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&search_url, Some(&headers), "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(items) = json["items"].as_array() {
                for r in items.iter().take(max_results) {
                    let name = r["full_name"].as_str().unwrap_or("").to_string();
                    let description = r["description"].as_str().unwrap_or("").to_string();
                    let html_url = r["html_url"].as_str().unwrap_or("").to_string();
                    let stars = r["stargazers_count"].as_u64().unwrap_or(0);
                    let language = r["language"].as_str().unwrap_or("").to_string();
                    let updated = r["updated_at"].as_str().map(|s| s.to_string());

                    let content = format!(
                        "⭐ {} stars | 📝 {} | {}",
                        stars, language, description
                    );

                    results.push(WebSearchResult {
                        title: name,
                        url: html_url,
                        content: Some(content),
                        score: Some(stars as f64 / 100000.0).map(|s| s.min(1.0)),
                        highlights: None,
                        raw_content: None,
                        source: Some("github".to_string()),
                        domain: Some("github.com".to_string()),
                        published_date: updated,
                        favicon: None,
                    });
                }
            }
        }
    }

    // Also search code when topic is explicitly code-related
    if topic == "code" {
    let code_url = format!(
        "https://api.github.com/search/code?q={}&per_page={}",
        query_encoded, max_results.min(5)
    );

    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) = http.fetch(&code_url, Some(&headers), "bypass").await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
            if let Some(items) = json["items"].as_array() {
                for r in items.iter().take(5) {
                    let path = r["path"].as_str().unwrap_or("").to_string();
                    let repo = r["repository"]["full_name"].as_str().unwrap_or("").to_string();
                    let html_url = r["html_url"].as_str().unwrap_or("").to_string();
                    let score_val = r["score"].as_f64().unwrap_or(0.0);

                    let title = format!("{}/{}", repo, path);
                    let content = r["text_matches"].as_array()
                        .map(|tm| tm.iter().take(2).map(|m| m["fragment"].as_str().unwrap_or("")).collect::<Vec<_>>().join(" ... "))
                        .unwrap_or_default();

                    results.push(WebSearchResult {
                        title,
                        url: html_url,
                        content: if content.is_empty() { None } else { Some(content) },
                        score: Some(score_val),
                        highlights: None,
                        raw_content: None,
                        source: Some("github".to_string()),
                        domain: Some("github.com".to_string()),
                        published_date: None,
                        favicon: None,
                    });
                }
            }
        }
    }
    } // end if topic == "code"

    Ok(("github".to_string(), results, None))
}

// ─── Hacker News Engine (Algolia API) ─────────────────────────

/// Search Hacker News via Algolia's free API. Returns (engine_name, results, answer).
/// API: https://hn.algolia.com/api/v1/search?query={q}&tags=story&hitsPerPage={limit}
async fn search_hackernews(
    query: &str,
    max_results: usize,
    http: &HttpClient,
    time_range: &str,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let hits = max_results.min(30);
    // Add time range filter via numericFilters (unix timestamp)
    let time_filter = chrono::Utc::now().timestamp() - match time_range {
        "day" => 86400,
        "week" => 604800,
        "month" => 2592000,
        "year" => 31536000,
        _ => 0,
    };
    let url = if time_filter > 0 {
        format!(
            "https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage={}&numericFilters=created_at_i>{}",
            query_encoded, hits, time_filter
        )
    } else {
        format!(
            "https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage={}",
            query_encoded, hits
        )
    };

    match http.fetch(&url, None, "bypass").await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("HN parse error: {}", e))?;

            let mut results = Vec::new();

            if let Some(hits_arr) = json["hits"].as_array() {
                for hit in hits_arr.iter().take(max_results) {
                    let title = hit["title"].as_str().unwrap_or("").to_string();
                    let url_str = hit["url"].as_str()
                        .unwrap_or(&format!("https://news.ycombinator.com/item?id={}", hit["objectID"].as_str().unwrap_or("")))
                        .to_string();
                    let author = hit["author"].as_str().unwrap_or("").to_string();
                    let points = hit["points"].as_u64().unwrap_or(0);
                    let num_comments = hit["num_comments"].as_u64().unwrap_or(0);
                    let created_at = hit["created_at"].as_str().unwrap_or("").to_string();

                    let content = format!(
                        "⬆️ {} points | 💬 {} comments | by {}",
                        points, num_comments, author
                    );

                    results.push(WebSearchResult {
                        title,
                        url: url_str,
                        content: Some(content),
                        score: Some((points as f64 / 500.0).min(1.0)),
                        highlights: None,
                        raw_content: None,
                        source: Some("hackernews".to_string()),
                        domain: Some("news.ycombinator.com".to_string()),
                        published_date: Some(created_at),
                        favicon: None,
                    });
                }
            }

            Ok(("hackernews".to_string(), results, None))
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => {
            // Bypass mode should never return Cached; treat as error
            Err("HN API: unexpected cached response in bypass mode".into())
        }
        Err(e) => Err(format!("HN API error: {}", e)),
    }
}

// ─── Stack Overflow Engine (StackExchange API) ────────────────

/// Search Stack Overflow via StackExchange API (free, 10K requests/day with API key).
/// Returns (engine_name, results, answer).
/// API: https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle={q}&site=stackoverflow
async fn search_stackoverflow(
    query: &str,
    max_results: usize,
    http: &HttpClient,
) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    let query_encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
    let pagesize = max_results.min(20);
    // Use 'all' instead of 'intitle' to search title+body for better relevance
    let url = format!(
        "https://api.stackexchange.com/2.3/search/advanced?order=desc&sort=relevance&q={}&site=stackoverflow&pagesize={}&filter=withbody",
        query_encoded, pagesize
    );

    match http.fetch(&url, None, "bypass").await {
        Ok(http_mod::FetchOutcome::Response(resp, _, _)) => {
            let json: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("SO parse error: {}", e))?;

            let mut results = Vec::new();

            if let Some(items) = json["items"].as_array() {
                for item in items.iter().take(max_results) {
                    let title = item["title"].as_str().unwrap_or("").to_string();
                    let link = item["link"].as_str().unwrap_or("").to_string();
                    let score = item["score"].as_i64().unwrap_or(0);
                    let answer_count = item["answer_count"].as_u64().unwrap_or(0);
                    let is_answered = item["is_answered"].as_bool().unwrap_or(false);
                    let tags: Vec<String> = item["tags"].as_array()
                        .map(|arr| arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    let created = item["creation_date"].as_i64().unwrap_or(0);
                    let view_count = item["view_count"].as_u64().unwrap_or(0);

                    // Convert unix timestamp to ISO date
                    let date_str = if created > 0 {
                        let dt = chrono::DateTime::from_timestamp(created, 0)
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_default();
                        dt
                    } else {
                        String::new()
                    };

                    let answered_marker = if is_answered { "✅" } else { "⏳" };
                    let content = format!(
                        "{} {} score | {} answers | 👁️ {} views | tags: {}",
                        answered_marker, score, answer_count, view_count, tags.join(", ")
                    );

                    results.push(WebSearchResult {
                        title,
                        url: link,
                        content: Some(content),
                        score: Some((score as f64 / 100.0).clamp(0.0, 1.0)),
                        highlights: None,
                        raw_content: None,
                        source: Some("stackoverflow".to_string()),
                        domain: Some("stackoverflow.com".to_string()),
                        published_date: if date_str.is_empty() { None } else { Some(date_str) },
                        favicon: None,
                    });
                }
            }

            Ok(("stackoverflow".to_string(), results, None))
        }
        Ok(http_mod::FetchOutcome::Cached(_)) => {
            // Bypass mode should never return Cached; treat as error
            Err("SO API: unexpected cached response in bypass mode".into())
        }
        Err(e) => Err(format!("SO API error: {}", e)),
    }
}

pub async fn web_scrape(input: WebScrapeInput) -> Result<WebScrapeOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;

    // Determine provider: explicit input, or browser.default from settings
    let provider = input.provider.as_deref().unwrap_or(&settings.browser.default);

    match provider {
        "lightpanda" => web_scrape_lightpanda(&input, &settings).await,
        "obscura" => web_scrape_obscura(&input, &settings).await,
        _ => web_scrape_default(&input, &settings).await,
    }
}

/// Scrape using plain HTTP + html-to-markdown-rs (default provider)
async fn web_scrape_default(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    let cache_dir = http_mod::resolve_cache_dir(settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let body = match http.fetch(&input.url, None, "bypass").await {
        Ok(outcome) => {
            let http_mod::FetchOutcome::Response(resp, _, _) = outcome else {
                unreachable!("bypass cache mode never returns Cached")
            };
            if resp.status < 200 || resp.status >= 400 {
                return Err(format!("HTTP {} for URL: {}", resp.status, input.url));
            }
            resp.body_text
        }
        Err(e) => return Err(format!("Scrape failed: {}", e)),
    };

    extract_scrape_output(&input.url, &body, "default", input.formats.as_deref())
}

/// Scrape using Lightpanda headless browser (JS rendering)
async fn web_scrape_lightpanda(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    let lp_settings = &settings.browser.lightpanda;
    if !lp_settings.enabled {
        return Err(
            "Lightpanda is not enabled. Set browser.lightpanda.enabled=true in settings.yml to use provider='lightpanda'"
                .into(),
        );
    }

    let lightpanda = crate::lightpanda::LightpandaManager::new(lp_settings);
    let obey_robots = lp_settings.obey_robots;
    let dump_format = "markdown";
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");

    let body = lightpanda
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            input.include_frames.unwrap_or(false),
            input.wait_selector.as_deref(),
        )
        .await
        .map_err(|e| format!("Lightpanda scrape failed: {}", e))?;

    extract_scrape_output(&input.url, &body, "lightpanda", input.formats.as_deref())
}

/// Scrape using Obscura headless browser (JS rendering)
async fn web_scrape_obscura(
    input: &WebScrapeInput,
    settings: &crate::types::Settings,
) -> Result<WebScrapeOutput, String> {
    let obs_settings = &settings.browser.obscura;
    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use provider='obscura'"
                .into(),
        );
    }

    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let obey_robots = obs_settings.obey_robots;
    let dump_format = "markdown";
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");

    let body = obscura
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            input.include_frames.unwrap_or(false),
            input.wait_selector.as_deref(),
        )
        .await
        .map_err(|e| format!("Obscura scrape failed: {}", e))?;

    extract_scrape_output(&input.url, &body, "obscura", input.formats.as_deref())
}

fn extract_scrape_output(
    url: &str,
    body: &str,
    _provider: &str,
    _formats: Option<&[String]>,
) -> Result<WebScrapeOutput, String> {
    let doc = scraper::Html::parse_document(body);

    let title = scraper::Selector::parse("title")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty());

    let description = scraper::Selector::parse("meta[name='description']")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.attr("content").map(|s| s.to_string()));

    let og_title = scraper::Selector::parse("meta[property='og:title']")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.attr("content").map(|s| s.to_string()));

    let og_description = scraper::Selector::parse("meta[property='og:description']")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.attr("content").map(|s| s.to_string()));

    let mut headings = Vec::new();
    for tag in &["h1", "h2", "h3"] {
        if let Ok(sel) = scraper::Selector::parse(tag) {
            for el in doc.select(&sel) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    headings.push(text);
                }
            }
        }
    }

    let links_count = scraper::Selector::parse("a[href]")
        .ok()
        .map(|sel| doc.select(&sel).count())
        .unwrap_or(0);

    let markdown = {
        let converted = html_to_markdown_rs::convert(body, None)
            .ok()
            .and_then(|r| r.content)
            .filter(|s: &String| !s.trim().is_empty());
        converted.unwrap_or_else(|| {
            let main_content: String = doc.root_element().text().collect::<String>();
            main_content
                .split_whitespace()
                .take(2000)
                .collect::<Vec<_>>()
                .join(" ")
        })
    };

    Ok(WebScrapeOutput {
        success: true,
        url: url.to_string(),
        title,
        markdown: Some(markdown),
        metadata: Some(ScrapeStructuredData {
            description,
            og_title,
            og_description,
            links_count,
            headings,
        }),
        meta: ScrapeMeta {
            url: url.to_string(),
            status: 200,
            content_type: None,
            elapsed_ms: 0,
            js_rendered: false,
        },
    })
}

pub async fn web_crawl(input: WebCrawlInput) -> Result<WebCrawlOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;

    // Use browser.default from settings
    let provider = &settings.browser.default;

    match provider.as_str() {
        "lightpanda" => web_crawl_lightpanda(&input, &settings).await,
        "obscura" => web_crawl_obscura(&input, &settings).await,
        _ => Err(
            "web.crawl requires a headless browser. Set browser.default to 'lightpanda' or 'obscura' in settings.yml"
                .into(),
        ),
    }
}

async fn web_crawl_lightpanda(
    input: &WebCrawlInput,
    settings: &crate::types::Settings,
) -> Result<WebCrawlOutput, String> {
    let lp_settings = &settings.browser.lightpanda;
    if !lp_settings.enabled {
        return Err(
            "Lightpanda is not enabled. Set browser.lightpanda.enabled=true in settings.yml"
                .into(),
        );
    }

    let lightpanda = crate::lightpanda::LightpandaManager::new(lp_settings);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);
    let obey_robots = input.obey_robots.unwrap_or(lp_settings.obey_robots);
    let dump_format = input.dump_format.as_deref().unwrap_or("markdown");
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let include_frames = input.include_frames.unwrap_or(false);
    let wait_selector = input.wait_selector.as_deref();

    let content = lightpanda
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            include_frames,
            wait_selector,
        )
        .await
        .map_err(|e| format!("Lightpanda fetch failed: {}", e))?;

    let title = {
        let doc = scraper::Html::parse_document(&content);
        scraper::Selector::parse("title")
            .ok()
            .and_then(|sel| doc.select(&sel).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
    };

    let mut pages = vec![CrawledPage {
        url: input.url.clone(),
        title,
        content,
        depth: 0,
        status: "ok".to_string(),
    }];

    if max_depth > 0 {
        let base_url = url::Url::parse(&input.url)
            .map_err(|e| format!("Invalid URL '{}': {}", input.url, e))?;
        let base_host = base_url.host_str().unwrap_or("").to_string();

        let mut queue: std::collections::VecDeque<(String, i32)> =
            std::collections::VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        visited.insert(input.url.clone());

        {
            let doc = scraper::Html::parse_document(&pages[0].content);
            let sel = scraper::Selector::parse("a[href]").expect("valid selector");
            for url_str in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                if !visited.contains(&url_str) {
                    visited.insert(url_str.clone());
                    queue.push_back((url_str, 1));
                }
            }
        }

        while let Some((url_str, depth)) = queue.pop_front() {
            if pages.len() >= max_pages as usize {
                break;
            }

            match lightpanda
                .fetch_with_all_options(
                    &url_str,
                    dump_format,
                    obey_robots,
                    wait_until,
                    include_frames,
                    wait_selector,
                )
                .await
            {
                Ok(content) => {
                    let title = {
                        let doc = scraper::Html::parse_document(&content);
                        scraper::Selector::parse("title")
                            .ok()
                            .and_then(|sel| doc.select(&sel).next())
                            .map(|el| el.text().collect::<String>().trim().to_string())
                    };

                    if depth < max_depth {
                        let doc = scraper::Html::parse_document(&content);
                        let sel = scraper::Selector::parse("a[href]").expect("valid selector");
                        for link_url in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                            if !visited.contains(&link_url)
                                && pages.len() + queue.len() < max_pages as usize
                            {
                                visited.insert(link_url.clone());
                                queue.push_back((link_url, depth + 1));
                            }
                        }
                    }

                    pages.push(CrawledPage {
                        url: url_str,
                        title,
                        content,
                        depth,
                        status: "ok".to_string(),
                    });
                }
                Err(e) => {
                    pages.push(CrawledPage {
                        url: url_str,
                        title: None,
                        content: format!("Error: {}", e),
                        depth,
                        status: "error".to_string(),
                    });
                }
            }
        }
    }

    let count = pages.len();
    Ok(WebCrawlOutput {
        success: true,
        start_url: input.url.clone(),
        pages,
        count,
        meta: WebCrawlMeta {
            provider: "lightpanda".to_string(),
            max_depth,
            max_pages,
            obey_robots,
            dump_format: dump_format.to_string(),
            wait_until: wait_until.to_string(),
            include_frames,
        },
    })
}

async fn web_crawl_obscura(
    input: &WebCrawlInput,
    settings: &crate::types::Settings,
) -> Result<WebCrawlOutput, String> {
    let obs_settings = &settings.browser.obscura;
    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use web.crawl"
                .into(),
        );
    }

    let obscura = crate::obscura::ObscuraManager::new(obs_settings);

    let max_depth = input.max_depth.unwrap_or(2);
    let max_pages = input.max_pages.unwrap_or(20);
    let obey_robots = input.obey_robots.unwrap_or(obs_settings.obey_robots);
    let dump_format = input.dump_format.as_deref().unwrap_or("markdown");
    let wait_until = input.wait_until.as_deref().unwrap_or("networkidle");
    let include_frames = input.include_frames.unwrap_or(false);
    let wait_selector = input.wait_selector.as_deref();

    let content = obscura
        .fetch_with_all_options(
            &input.url,
            dump_format,
            obey_robots,
            wait_until,
            include_frames,
            wait_selector,
        )
        .await
        .map_err(|e| format!("Obscura fetch failed: {}", e))?;

    let title = {
        let doc = scraper::Html::parse_document(&content);
        scraper::Selector::parse("title")
            .ok()
            .and_then(|sel| doc.select(&sel).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
    };

    let mut pages = vec![CrawledPage {
        url: input.url.clone(),
        title,
        content,
        depth: 0,
        status: "ok".to_string(),
    }];

    if max_depth > 0 {
        let base_url = url::Url::parse(&input.url)
            .map_err(|e| format!("Invalid URL '{}': {}", input.url, e))?;
        let base_host = base_url.host_str().unwrap_or("").to_string();

        let mut queue: std::collections::VecDeque<(String, i32)> =
            std::collections::VecDeque::new();
        let mut visited = std::collections::HashSet::new();
        visited.insert(input.url.clone());

        {
            let doc = scraper::Html::parse_document(&pages[0].content);
            let sel = scraper::Selector::parse("a[href]").expect("valid selector");
            for url_str in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                if !visited.contains(&url_str) {
                    visited.insert(url_str.clone());
                    queue.push_back((url_str, 1));
                }
            }
        }

        while let Some((url_str, depth)) = queue.pop_front() {
            if pages.len() >= max_pages as usize {
                break;
            }

            match obscura
                .fetch_with_all_options(
                    &url_str,
                    dump_format,
                    obey_robots,
                    wait_until,
                    include_frames,
                    wait_selector,
                )
                .await
            {
                Ok(content) => {
                    let title = {
                        let doc = scraper::Html::parse_document(&content);
                        scraper::Selector::parse("title")
                            .ok()
                            .and_then(|sel| doc.select(&sel).next())
                            .map(|el| el.text().collect::<String>().trim().to_string())
                    };

                    if depth < max_depth {
                        let doc = scraper::Html::parse_document(&content);
                        let sel = scraper::Selector::parse("a[href]").expect("valid selector");
                        for link_url in extract_internal_links(&doc, &sel, &base_url, &base_host) {
                            if !visited.contains(&link_url)
                                && pages.len() + queue.len() < max_pages as usize
                            {
                                visited.insert(link_url.clone());
                                queue.push_back((link_url, depth + 1));
                            }
                        }
                    }

                    pages.push(CrawledPage {
                        url: url_str,
                        title,
                        content,
                        depth,
                        status: "ok".to_string(),
                    });
                }
                Err(e) => {
                    pages.push(CrawledPage {
                        url: url_str,
                        title: None,
                        content: format!("Error: {}", e),
                        depth,
                        status: "error".to_string(),
                    });
                }
            }
        }
    }

    let count = pages.len();
    Ok(WebCrawlOutput {
        success: true,
        start_url: input.url.clone(),
        pages,
        count,
        meta: WebCrawlMeta {
            provider: "obscura".to_string(),
            max_depth,
            max_pages,
            obey_robots,
            dump_format: dump_format.to_string(),
            wait_until: wait_until.to_string(),
            include_frames,
        },
    })
}

/// Content cleaning selectors (from CRW porting analysis).
/// These selectors target navigation, ads, cookie banners, and boilerplate.
/// Remove boilerplate text patterns from content using word-boundary matching.
/// Uses space-delimited tokenization to avoid corrupting words like "cookies" → "".
fn remove_boilerplate(text: &str) -> String {
    let boilerplate_phrases = [
        "privacy policy", "terms of service", "all rights reserved",
        "subscribe to", "sign up for", "newsletter",
        "follow us on", "share this", "tweet this",
        "sponsored content", "loading...", "click here to", "read more:",
    ];
    let boilerplate_words = ["cookie", "copyright", "advertisement"];
    let mut result = text.to_string();
    // Replace full phrases first
    for phrase in &boilerplate_phrases {
        result = result.replace(phrase, "");
    }
    // Replace single words with word-boundary check
    for word in &boilerplate_words {
        // Match word with space before or after, or at start/end
        let mut new_result = String::with_capacity(result.len());
        for token in result.split(' ') {
            if token.eq_ignore_ascii_case(word) {
                continue; // skip the word
            }
            if !new_result.is_empty() {
                new_result.push(' ');
            }
            new_result.push_str(token);
        }
        result = new_result;
    }
    // Collapse multiple spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    result.trim().to_string()
}

/// Detect content type from page structure and metadata.
fn detect_content_type(doc: &scraper::Html) -> Option<String> {
    // Check JSON-LD type
    if let Ok(sel) = scraper::Selector::parse("script[type='application/ld+json']") {
        for el in doc.select(&sel) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&el.text().collect::<String>()) {
                if let Some(t) = json["@type"].as_str() {
                    return Some(t.to_string());
                }
            }
        }
    }

    // Check OG type
    if let Ok(sel) = scraper::Selector::parse("meta[property='og:type']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(t) = el.attr("content") {
                return Some(t.to_string());
            }
        }
    }

    None // Only expose structured types from JSON-LD / OG; avoid fragile title heuristics
}

/// Detect language from meta tags.
fn detect_language(doc: &scraper::Html) -> Option<String> {
    // Check meta lang attribute
    if let Ok(sel) = scraper::Selector::parse("html[lang]") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.attr("lang") {
                return Some(lang.to_string());
            }
        }
    }
    // Check meta http-equiv
    if let Ok(sel) = scraper::Selector::parse("meta[http-equiv='content-language']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.attr("content") {
                return Some(lang.to_string());
            }
        }
    }
    // Check og:locale
    if let Ok(sel) = scraper::Selector::parse("meta[property='og:locale']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.attr("content") {
                return Some(lang.split('_').next().unwrap_or(lang).to_string());
            }
        }
    }
    None
}

/// Extract structured content from a URL using Obscura.
/// Supports full extraction (text, metadata, links, images, structured data)
/// or selector-based extraction for specific elements.
/// Batch mode: if `urls` is provided, processes multiple URLs in parallel.
pub async fn web_extract(input: WebExtractInput) -> Result<WebExtractOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let obs_settings = &settings.browser.obscura;

    if !obs_settings.enabled {
        return Err(
            "Obscura is not enabled. Set browser.obscura.enabled=true in settings.yml to use web.extract"
                .into(),
        );
    }

    let clean_content = input.clean_content.unwrap_or(false);
    let start = std::time::Instant::now();
    let url = &input.url;
    let obscura = crate::obscura::ObscuraManager::new(obs_settings);
    let wait_until = "networkidle";

    // Fetch the page with Obscura (JS rendering)
    let html = obscura
        .fetch_with_all_options(
            url,
            "html",
            false,
            wait_until,
            false,
            input.wait_selector.as_deref(),
        )
        .await
        .map_err(|e| format!("Obscura fetch failed: {}", e))?;

    let doc = scraper::Html::parse_document(&html);

    // Extract title
    let title = scraper::Selector::parse("title")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty());

    // Extract main text content (with optional cleaning)
    let raw_content = extract_main_text(&doc);
    let content = if clean_content {
        remove_boilerplate(&raw_content)
    } else {
        raw_content
    };

    // If a query is provided, chunk-score the content with BM25 for relevance ranking
    let content = if let Some(ref query) = input.query {
        if !query.trim().is_empty() && !content.is_empty() {
            // Split content into paragraph chunks on double-newlines
            let chunks: Vec<String> = content
                .split("\n\n")
                .map(|s| s.trim().to_string())
                .filter(|s| s.len() > 50)
                .collect();

            if chunks.len() > 3 {
                // BM25-score all chunks, keep only those with meaningful relevance
                let scored = bm25_score_chunks(&chunks, query, chunks.len());
                let filtered: Vec<(usize, &str)> = scored
                    .iter()
                    .filter(|c| c.score > BM25_MIN_THRESHOLD)
                    .map(|c| (c.index, c.content.as_str()))
                    .collect();
                if !filtered.is_empty() {
                    // Reassemble in original reading order
                    let mut ordered = filtered;
                    ordered.sort_by_key(|(i, _)| *i);
                    ordered.into_iter().map(|(_, c)| c).collect::<Vec<_>>().join("\n\n")
                } else {
                    content
                }
            } else {
                content
            }
        } else {
            content
        }
    } else {
        content
    };

    // Generate markdown
    let markdown = html_to_markdown_rs::convert(&html, None)
        .ok()
        .and_then(|r| r.content)
        .filter(|s| !s.trim().is_empty());

    // Extract metadata (with enhancements)
    let mut metadata = extract_page_metadata(&doc);
    metadata.language = detect_language(&doc);
    metadata.content_type = detect_content_type(&doc);
    // reading_time_minutes is already computed in extract_page_metadata

    // Extract structured data if requested
    let structured_data = if input.structured_data.unwrap_or(false) {
        extract_structured_data(&doc)
    } else {
        None
    };

    // Extract links if requested
    let links = if input.extract_links.unwrap_or(false) {
        extract_page_links(&doc)
    } else {
        None
    };

    // Extract images if requested
    let images = if input.extract_images.unwrap_or(false) {
        extract_page_images(&doc)
    } else {
        None
    };

    // Extract elements by selector if provided
    let elements = if let Some(selectors) = &input.selectors {
        extract_by_selectors(&doc, selectors)
    } else {
        None
    };

    // Include raw HTML if requested
    let html_output = if input.include_html.unwrap_or(false) {
        Some(html.clone())
    } else {
        None
    };

    let elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(WebExtractOutput {
        success: true,
        url: url.clone(),
        title,
        content: Some(content),
        markdown,
        html: html_output,
        metadata: Some(metadata),
        structured_data,
        links,
        images,
        elements,
        meta: ExtractMeta {
            url: url.clone(),
            provider: "obscura".into(),
            js_rendered: true,
            elapsed_ms,
        },
    })
}

/// Extract main text content from the document body
fn extract_main_text(doc: &scraper::Html) -> String {
    // Try to find main content area first
    let main_selectors = ["main", "article", "[role='main']", ".content", "#content"];
    for selector_str in &main_selectors {
        if let Ok(sel) = scraper::Selector::parse(selector_str) {
            if let Some(main_el) = doc.select(&sel).next() {
                let text = main_el.text().collect::<String>();
                let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if cleaned.len() > 100 {
                    return cleaned;
                }
            }
        }
    }
    // Fallback to body
    if let Ok(sel) = scraper::Selector::parse("body") {
        if let Some(body_el) = doc.select(&sel).next() {
            let text = body_el.text().collect::<String>();
            return text.split_whitespace().collect::<Vec<_>>().join(" ");
        }
    }
    // Last resort: root element
    let text: String = doc.root_element().text().collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract page metadata (description, OpenGraph, author, publish date)
fn extract_page_metadata(doc: &scraper::Html) -> ExtractMetadata {
    let description = doc.select(&scraper::Selector::parse("meta[name='description']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let og_title = doc.select(&scraper::Selector::parse("meta[property='og:title']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let og_description = doc.select(&scraper::Selector::parse("meta[property='og:description']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let og_image = doc.select(&scraper::Selector::parse("meta[property='og:image']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let author = doc.select(&scraper::Selector::parse("meta[name='author']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .map(|s| s.to_string());

    let publish_date = doc.select(&scraper::Selector::parse("meta[property='article:published_time']").unwrap())
        .next()
        .and_then(|el| el.attr("content"))
        .or_else(|| {
            doc.select(&scraper::Selector::parse("meta[name='date']").unwrap())
                .next()
                .and_then(|el| el.attr("content"))
        })
        .map(|s| s.to_string());

    let body_text = doc.root_element().text().collect::<String>();
    let word_count = body_text.split_whitespace().count();

    ExtractMetadata {
        description,
        og_title,
        og_description,
        og_image,
        author,
        publish_date,
        word_count,
        reading_time_minutes: Some((word_count as u32 / 200).max(1)),
        language: None, // populated by caller via detect_language()
        content_type: None, // populated by caller via detect_content_type()
    }
}

/// Extract structured data (JSON-LD and OpenGraph)
fn extract_structured_data(doc: &scraper::Html) -> Option<StructuredData> {
    // Extract JSON-LD
    let json_ld: Vec<serde_json::Value> = doc
        .select(&scraper::Selector::parse("script[type='application/ld+json']").unwrap())
        .filter_map(|el| {
            let text = el.text().collect::<String>();
            serde_json::from_str(&text).ok()
        })
        .collect();

    // Extract OpenGraph
    let mut opengraph: HashMap<String, String> = HashMap::new();
    for prop in &["og:title", "og:description", "og:image", "og:url", "og:type", "og:site_name"] {
        if let Ok(sel) = scraper::Selector::parse(&format!("meta[property='{}']", prop)) {
            if let Some(el) = doc.select(&sel).next() {
                if let Some(content) = el.attr("content") {
                    opengraph.insert(prop.to_string(), content.to_string());
                }
            }
        }
    }

    if json_ld.is_empty() && opengraph.is_empty() {
        None
    } else {
        Some(StructuredData {
            json_ld: if json_ld.is_empty() { None } else { Some(json_ld) },
            opengraph: if opengraph.is_empty() { None } else { Some(opengraph) },
        })
    }
}

/// Extract all links from the page
fn extract_page_links(doc: &scraper::Html) -> Option<Vec<ExtractedLink>> {
    let mut links = Vec::new();
    if let Ok(sel) = scraper::Selector::parse("a[href]") {
        for el in doc.select(&sel) {
            if let Some(href) = el.attr("href") {
                let text = el.text().collect::<String>().trim().to_string();
                let rel = el.attr("rel").map(|s| s.to_string());
                links.push(ExtractedLink {
                    url: href.to_string(),
                    text,
                    rel,
                });
            }
        }
    }
    if links.is_empty() { None } else { Some(links) }
}

/// Extract all images from the page
fn extract_page_images(doc: &scraper::Html) -> Option<Vec<ExtractedImage>> {
    let mut images = Vec::new();
    if let Ok(sel) = scraper::Selector::parse("img[src]") {
        for el in doc.select(&sel) {
            if let Some(src) = el.attr("src") {
                let alt = el.attr("alt").map(|s| s.to_string());
                let width = el.attr("width").map(|s| s.to_string());
                let height = el.attr("height").map(|s| s.to_string());
                images.push(ExtractedImage {
                    url: src.to_string(),
                    alt,
                    width,
                    height,
                });
            }
        }
    }
    if images.is_empty() { None } else { Some(images) }
}

/// Extract elements by CSS selectors
fn extract_by_selectors(doc: &scraper::Html, selectors: &[String]) -> Option<Vec<ExtractedElement>> {
    let mut elements = Vec::new();
    for selector_str in selectors {
        if let Ok(sel) = scraper::Selector::parse(selector_str) {
            for el in doc.select(&sel) {
                let html = el.html();
                let text = el.text().collect::<String>().trim().to_string();
                elements.push(ExtractedElement {
                    selector: selector_str.clone(),
                    html,
                    text,
                });
            }
        }
    }
    if elements.is_empty() { None } else { Some(elements) }
}

/// Discover URLs on a website by analyzing sitemap and links.
pub async fn web_map(input: WebMapInput) -> Result<WebMapOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let base_url = input.url.trim_end_matches('/');
    let sitemap_url = format!("{}/sitemap.xml", base_url);

    let mut links: Vec<WebMapLink> = Vec::new();
    let mut sitemap_fetched = false;

    // Try sitemap.xml
    if let Ok(http_mod::FetchOutcome::Response(resp, _, _)) =
        http.fetch(&sitemap_url, None, "bypass").await
    {
        // Only treat as fetched if the response is a real sitemap (HTTP 200
        // and body contains <urlset> or <sitemapindex>). A 404 or HTML error
        // page would otherwise silently produce an empty-but-"success" result.
        if resp.status >= 200 && resp.status < 400 {
            let body = &resp.body_text;
            if body.contains("<urlset") || body.contains("<sitemapindex") {
                sitemap_fetched = true;
                let doc = scraper::Html::parse_document(body);
                if let Ok(sel) = scraper::Selector::parse("url") {
                    for el in doc.select(&sel) {
                        if let Ok(loc_sel) = scraper::Selector::parse("loc") {
                            if let Some(loc) = el.select(&loc_sel).next() {
                                let url_str = loc.text().collect::<String>().trim().to_string();
                                if !url_str.is_empty() && !links.iter().any(|l| l.url == url_str) {
                                    let title = scraper::Selector::parse("news\\:title")
                                        .or_else(|_| scraper::Selector::parse("title"))
                                        .ok()
                                        .and_then(|ts| el.select(&ts).next())
                                        .map(|t| t.text().collect::<String>());
                                    links.push(WebMapLink {
                                        url: url_str,
                                        title,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Filter by search term if provided
    if let Some(ref search) = input.search {
        let search_lower = search.to_lowercase();
        links.retain(|link| {
            link.url.to_lowercase().contains(&search_lower)
                || link
                    .title
                    .as_ref()
                    .is_some_and(|t| t.to_lowercase().contains(&search_lower))
        });
    }

    let limit = input.limit.unwrap_or(100) as usize;
    links.truncate(limit);
    let count = links.len();

    Ok(WebMapOutput {
        success: sitemap_fetched,
        url: input.url,
        count,
        links,
        meta: WebMapMeta {
            provider: "sitemap-parser".to_string(),
            limit,
        },
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello, World! This is a test.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
        assert!(tokens.contains(&"is".to_string())); // 2-char tokens are kept
        // Single-char tokens filtered out
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn bm25_returns_sorted_by_relevance() {
        let chunks = vec![
            "Rust is a systems programming language".to_string(),
            "Python is great for machine learning".to_string(),
            "Rust async runtime tokio for concurrent programs".to_string(),
        ];
        let scored = bm25_score_chunks(&chunks, "rust async", 2);
        assert_eq!(scored.len(), 2);
        // First result should mention both "rust" and "async"
        assert!(scored[0].content.contains("Rust"));
        assert!(scored[0].score >= scored[1].score);
    }

    #[test]
    fn bm25_empty_input() {
        let scored = bm25_score_chunks(&[], "test", 5);
        assert!(scored.is_empty());
    }

    #[test]
    fn bm25_empty_query() {
        let chunks = vec!["hello".to_string()];
        let scored = bm25_score_chunks(&chunks, "", 5);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].score, 0.0);
    }

    #[test]
    fn jaccard_similarity_identical() {
        assert!((jaccard_similarity("hello world", "hello world") - 1.0).abs() < 0.001);
    }

    #[test]
    fn jaccard_similarity_disjoint() {
        assert!((jaccard_similarity("hello", "world") - 0.0).abs() < 0.001);
    }

    #[test]
    fn jaccard_similarity_partial() {
        let sim = jaccard_similarity("hello world foo", "hello world bar");
        assert!(sim > 0.3 && sim < 0.8);
    }

    #[test]
    fn keyword_relevance_basic() {
        let score = keyword_relevance("Rust async tutorial", "Learn about async in Rust", "rust async");
        assert!(score > 0.5);
    }

    #[test]
    fn domain_authority_known() {
        assert!(domain_authority("github.com") > 0.9);
        assert!(domain_authority("stackoverflow.com") > 0.85);
    }

    #[test]
    fn domain_authority_unknown() {
        assert_eq!(domain_authority("random-blog.example.com"), 0.5);
    }

    #[test]
    fn semantic_dedup_removes_similar() {
        let mut results = vec![
            WebSearchResult {
                title: "How to learn Rust programming".to_string(),
                url: "https://a.com".to_string(),
                content: None, score: Some(0.9), highlights: None,
                raw_content: None, source: None, domain: None,
                published_date: None, favicon: None,
            },
            WebSearchResult {
                title: "How to learn Rust programming language".to_string(),
                url: "https://b.com".to_string(),
                content: None, score: Some(0.7), highlights: None,
                raw_content: None, source: None, domain: None,
                published_date: None, favicon: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://a.com");
    }

    #[test]
    fn text_density_basic() {
        let html = "<html><body><p>This is some text content.</p></body></html>";
        let density = text_density(html);
        assert!(density > 0.0 && density <= 1.0);
    }

    #[test]
    fn ddg_redirect_url_parses() {
        let url = "/l/?uddg=https%3A%2F%2Fexample.com%2Fpage&rut=abc123";
        let result = extract_ddg_redirect_url(url);
        assert_eq!(result.as_deref(), Some("https://example.com/page"));
    }

    #[test]
    fn ddg_redirect_url_none_for_normal() {
        assert!(extract_ddg_redirect_url("https://normal-url.com").is_none());
    }
}
