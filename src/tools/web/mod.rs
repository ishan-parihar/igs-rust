use crate::error::AppResult;
use crate::http::HttpClient;
use crate::tools::types::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

mod engines;
mod extractors;
mod readability;
mod scoring;
pub use engines::web_image_search;
pub use extractors::web_screenshot;
pub use extractors::{web_crawl, web_extract, web_map, web_scrape};
pub use readability::{extract_ddg_redirect_url, extract_semantic_excerpt};
pub(crate) use scoring::*;

pub async fn web_search(
    input: WebSearchInput,
    http: Arc<HttpClient>,
    settings: &crate::types::Settings,
) -> AppResult<WebSearchOutput> {
    let max_results: usize = input.max_results.unwrap_or(10) as usize;
    let depth = input.depth.as_deref().unwrap_or("fast");
    let topic = input.topic.as_deref().unwrap_or("general");
    let content_length = input.content_length.as_deref().unwrap_or("standard");
    let include_highlights = input.include_highlights.unwrap_or(false);
    let is_deep = depth == "deep";

    // Check cache first (skip for deep mode or explicit engines)
    if !is_deep && input.engines.is_none() {
        if let Some(cached) = cache_get(
            &input.query,
            topic,
            max_results,
            content_length,
            include_highlights,
            input.include_answer.unwrap_or(false),
        ) {
            return Ok(cached);
        }
    }

    // Determine which engines to use
    let engines = input
        .engines
        .clone()
        .unwrap_or_else(|| route_engines(topic, &input.query));

    let start = Instant::now();

    // Launch all engine queries in parallel
    let mut handles = Vec::new();

    for engine in &engines {
        match engine.as_str() {
            "duckduckgo" => {
                let q = input.query.clone();
                let obs_settings = settings.browser.obscura.clone();
                let include_answer = input.include_answer.unwrap_or(false);
                let http_clone = http.clone();
                let time_range_clone = input.time_range.clone().unwrap_or_default();
                handles.push(tokio::spawn(async move {
                    engines::search_duckduckgo(
                        &q,
                        max_results * 2,
                        include_answer,
                        &time_range_clone,
                        &obs_settings,
                        &http_clone,
                    )
                    .await
                }));
            }

            "wikipedia" => {
                let q = input.query.clone();
                let http_clone = http.clone();
                handles.push(tokio::spawn(async move {
                    engines::search_wikipedia(&q, (max_results / 2).max(3), &http_clone).await
                }));
            }
            "github" => {
                let q = input.query.clone();
                let http_clone = http.clone();
                let topic_clone = topic.to_string();
                handles.push(tokio::spawn(async move {
                    engines::search_github(&q, max_results, &http_clone, &topic_clone).await
                }));
            }
            "hackernews" => {
                let q = input.query.clone();
                let http_clone = http.clone();
                let time_range_clone = input.time_range.clone().unwrap_or_default();
                handles.push(tokio::spawn(async move {
                    engines::search_hackernews(&q, max_results, &http_clone, &time_range_clone)
                        .await
                }));
            }
            "stackoverflow" => {
                let q = input.query.clone();
                let http_clone = http.clone();
                handles.push(tokio::spawn(async move {
                    engines::search_stackoverflow(&q, max_results, &http_clone).await
                }));
            }
            "youtube" => {
                let q = input.query.clone();
                handles.push(tokio::spawn(async move {
                    engines::search_youtube(&q, max_results).await
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
        b.score
            .unwrap_or(0.0)
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
                        result.raw_content = Some(
                            html_to_markdown_rs::convert(&html, None)
                                .ok()
                                .and_then(|r| r.content)
                                .unwrap_or_default(),
                        );
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

    // Chunked content per source: split into paragraphs, score with BM25, return top-N
    if let Some(chunks_per_source) = input.chunks_per_source {
        let cps = chunks_per_source.max(1) as usize;
        for result in &mut deduped {
            let text = result
                .raw_content
                .as_deref()
                .or(result.content.as_deref())
                .unwrap_or("");
            if text.len() < 100 {
                continue;
            }
            let paragraphs = split_paragraphs(text);
            if paragraphs.is_empty() {
                continue;
            }
            let scored = bm25_score_chunks(&paragraphs, &input.query, cps);
            result.chunks = Some(
                scored
                    .into_iter()
                    .map(|c| ScoredChunkOutput {
                        content: c.content,
                        score: c.score,
                        index: c.index,
                    })
                    .collect(),
            );
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

    // Compute answer confidence using per-result scores + diversity + agreement
    let confidence = if answer.is_some() {
        Some(compute_answer_confidence(&deduped))
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
        cache_set(
            &input.query,
            topic,
            max_results,
            content_length,
            include_highlights,
            input.include_answer.unwrap_or(false),
            &output,
        );
    }

    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::readability::text_density;
    use super::*;
    use crate::tools::nlp::tokenize;
    use std::time::Duration;

    #[test]
    fn tokenize_basic() {
        let tokens = tokenize("Hello, World! This is a test.", 2, false);
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
        let score = keyword_relevance(
            "Rust async tutorial",
            "Learn about async in Rust",
            "rust async",
        );
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
                content: None,
                score: Some(0.9),
                highlights: None,
                raw_content: None,
                source: None,
                domain: None,
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "How to learn Rust programming language".to_string(),
                url: "https://b.com".to_string(),
                content: None,
                score: Some(0.7),
                highlights: None,
                raw_content: None,
                source: None,
                domain: None,
                published_date: None,
                favicon: None,
                chunks: None,
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

    #[test]
    fn extractive_answer_empty_results() {
        let results = vec![];
        assert!(extractive_answer(&results, "test query").is_none());
    }

    #[test]
    fn extractive_answer_empty_query() {
        let results = vec![WebSearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: Some("This is a long sentence about the test topic with enough content to score above the minimum threshold.".to_string()),
            score: Some(0.8),
            highlights: None,
            raw_content: None,
            source: None,
            domain: None,
            published_date: None,
            favicon: None,
                chunks: None,
        }];
        assert!(extractive_answer(&results, "").is_none());
    }

    #[test]
    fn extractive_answer_returns_relevant_sentences() {
        let results = vec![WebSearchResult {
            title: "Rust Programming".to_string(),
            url: "https://example.com".to_string(),
            content: Some("Rust is a systems programming language focused on safety and performance. Rust provides memory safety without garbage collection. The Rust compiler catches many bugs at compile time.".to_string()),
            score: Some(0.9),
            highlights: None,
            raw_content: None,
            source: None,
            domain: None,
            published_date: None,
            favicon: None,
                chunks: None,
        }];
        let answer = extractive_answer(&results, "rust programming language");
        assert!(answer.is_some());
        let text = answer.unwrap();
        assert!(text.to_lowercase().contains("rust"));
    }

    #[test]
    fn extractive_answer_prefers_raw_content() {
        let results = vec![WebSearchResult {
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            content: Some("Short snippet.".to_string()),
            score: Some(0.8),
            highlights: None,
            raw_content: Some("This is the full raw content with much more detail about the test topic that should be preferred over the short snippet because it has more sentences to score from.".to_string()),
            source: None,
            domain: None,
            published_date: None,
            favicon: None,
                chunks: None,
        }];
        let answer = extractive_answer(&results, "test topic");
        assert!(answer.is_some());
        // Should use raw_content, not the short snippet
        assert!(answer.unwrap().len() > 50);
    }

    #[test]
    fn jaccard_similarity_three_identical() {
        let sim = jaccard_similarity("hello world test", "hello world test");
        assert!((sim - 1.0).abs() < 0.01);
    }

    #[test]
    fn jaccard_similarity_completely_different() {
        let sim = jaccard_similarity("hello world", "foo bar baz");
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn jaccard_similarity_partial_overlap() {
        let sim = jaccard_similarity("hello world test", "hello world foo");
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn bm25_empty_query_zero_score() {
        let chunks = vec!["hello".to_string()];
        let scored = bm25_score_chunks(&chunks, "", 5);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].score, 0.0);
    }

    #[test]
    fn bm25_no_chunks() {
        let chunks: Vec<String> = vec![];
        let scored = bm25_score_chunks(&chunks, "test query", 5);
        assert!(scored.is_empty());
    }

    #[test]
    fn bm25_scores_relevant_higher() {
        let chunks = vec![
            "Rust is a programming language for systems programming".to_string(),
            "The weather is sunny today with clear skies".to_string(),
        ];
        let scored = bm25_score_chunks(&chunks, "rust programming", 2);
        assert_eq!(scored.len(), 2);
        // First chunk should score higher (contains both query terms)
        assert!(scored[0].score >= scored[1].score);
    }

    #[test]
    fn keyword_relevance_title_match() {
        let score = keyword_relevance("Rust Programming Guide", "Some content", "rust programming");
        assert!(score > 0.8); // Title match counts double
    }

    #[test]
    fn keyword_relevance_no_match() {
        let score = keyword_relevance("Weather Forecast", "Sunny skies", "rust programming");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn freshness_score_iso_date() {
        // Today's date should get 1.0
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        assert_eq!(freshness_score(Some(&today)), 1.0);
    }

    #[test]
    fn freshness_score_unknown() {
        assert_eq!(freshness_score(None), 0.5);
    }

    #[test]
    fn freshness_score_relative_time() {
        assert_eq!(freshness_score(Some("2 hours ago")), 1.0);
        assert_eq!(freshness_score(Some("last week")), 0.6);
        assert_eq!(freshness_score(Some("last month")), 0.4);
    }

    #[test]
    fn truncate_content_none() {
        assert!(truncate_content(None, "standard").is_none());
    }

    #[test]
    fn truncate_content_short() {
        let text = "Short text.";
        let result = truncate_content(Some(text), "standard").unwrap();
        assert_eq!(result, text);
    }

    #[test]
    fn truncate_content_long() {
        let text = "This is a very long text that should be truncated to a shorter version when using standard mode. It needs to be over 150 characters to actually trigger truncation in minimal mode. Adding more filler text to ensure we exceed the threshold. Even more padding here to make sure the test is robust. Extra words for safety.";
        let result = truncate_content(Some(text), "minimal").unwrap();
        assert!(result.len() < text.len());
        assert!(result.ends_with("..."));
    }

    #[test]
    fn route_engines_code_query() {
        let engines = route_engines("general", "rust compiler error");
        assert!(engines.contains(&"github".to_string()));
        assert!(engines.contains(&"stackoverflow".to_string()));
    }

    #[test]
    fn route_engines_news_query() {
        let engines = route_engines("general", "breaking news today");
        assert!(engines.contains(&"duckduckgo".to_string()));
        assert!(engines.contains(&"hackernews".to_string()));
    }

    #[test]
    fn route_engines_explicit_topic() {
        let engines = route_engines("news", "anything");
        assert!(engines.contains(&"duckduckgo".to_string()));
        assert!(engines.contains(&"hackernews".to_string()));
    }

    #[test]
    fn route_engines_video_intent() {
        let engines = route_engines("general", "how to make a video tutorial");
        // Base engines must still be present
        assert!(engines.contains(&"duckduckgo".to_string()));
        assert!(engines.contains(&"wikipedia".to_string()));
        assert!(engines.contains(&"hackernews".to_string()));
        // YouTube added on top for video intent
        assert!(engines.contains(&"youtube".to_string()));
    }

    #[test]
    fn cache_ttl_by_topic() {
        assert_eq!(cache_ttl("news"), Duration::from_secs(300));
        assert_eq!(cache_ttl("code"), Duration::from_secs(43_200));
        assert_eq!(cache_ttl("general"), Duration::from_secs(3_600));
    }

    // ─── extract_highlights tests ─────────────────────────────

    #[test]
    fn extract_highlights_empty_text() {
        let hl = extract_highlights("", "rust programming", 5);
        assert!(hl.is_empty());
    }

    #[test]
    fn extract_highlights_empty_query() {
        let hl = extract_highlights("This is some text.", "", 5);
        assert!(hl.is_empty());
    }

    #[test]
    fn extract_highlights_scoring() {
        let text = "Rust programming language is fast and safe. Python is also good. Rust has memory safety. Java is older.";
        let hl = extract_highlights(text, "rust programming", 5);
        // Sentences containing 'rust' or 'programming' should rank higher
        assert!(!hl.is_empty());
        assert!(
            hl[0].to_lowercase().contains("rust") || hl[0].to_lowercase().contains("programming")
        );
    }

    #[test]
    fn extract_highlights_max_limit() {
        let text = "First sentence about rust. Second sentence about rust. Third sentence about rust. Fourth sentence about rust. Fifth sentence about rust. Sixth sentence about rust.";
        let hl = extract_highlights(text, "rust", 2);
        assert!(hl.len() <= 2);
    }

    #[test]
    fn extract_highlights_filters_short_sentences() {
        // Sentences shorter than 30 chars are excluded
        let text =
            "Short. This is a longer sentence about rust programming that should be included.";
        let hl = extract_highlights(text, "rust", 5);
        assert!(!hl.is_empty());
        assert!(hl.iter().all(|s| s.len() > 30));
    }

    // ─── domain_dedup tests ───────────────────────────────────

    #[test]
    fn domain_dedup_removes_duplicates() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "Article 1".into(),
                url: "https://www.bbc.com/news/1".into(),
                content: Some("content1".into()),
                score: Some(0.5),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "Article 2".into(),
                url: "https://www.bbc.com/news/2".into(),
                content: Some("content2".into()),
                score: Some(0.8),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Article 2"); // higher score wins
    }

    #[test]
    fn domain_dedup_preserves_different_domains() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "From BBC".into(),
                url: "https://bbc.com/1".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "From Reuters".into(),
                url: "https://reuters.com/1".into(),
                content: Some("c".into()),
                score: Some(0.6),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("reuters.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 2); // different domains kept
    }

    #[test]
    fn domain_dedup_multipart_tld() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "BBC UK 1".into(),
                url: "https://www.bbc.co.uk/news/1".into(),
                content: Some("c".into()),
                score: Some(0.4),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("www.bbc.co.uk".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "BBC UK 2".into(),
                url: "https://bbc.co.uk/sport/1".into(),
                content: Some("c".into()),
                score: Some(0.9),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("bbc.co.uk".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
        ];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "BBC UK 2"); // higher score
    }

    #[test]
    fn domain_dedup_no_domain() {
        use super::WebSearchResult;
        let mut results = vec![WebSearchResult {
            title: "No domain".into(),
            url: "https://example.com/1".into(),
            content: Some("c".into()),
            score: Some(0.5),
            highlights: None,
            raw_content: None,
            source: Some("ddg".into()),
            domain: None,
            published_date: None,
            favicon: None,
            chunks: None,
        }];
        domain_dedup(&mut results);
        assert_eq!(results.len(), 1); // no domain = no dedup
    }

    // ─── semantic_dedup tests ──────────────────────────────────

    #[test]
    fn semantic_dedup_identical_titles() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "Rust programming language tutorial".into(),
                url: "https://a.com".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("a.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "Rust programming language tutorial".into(),
                url: "https://b.com".into(),
                content: Some("c".into()),
                score: Some(0.8),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("b.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://b.com"); // higher score wins
    }

    #[test]
    fn semantic_dedup_different_titles() {
        use super::WebSearchResult;
        let mut results = vec![
            WebSearchResult {
                title: "Rust programming".into(),
                url: "https://a.com".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("a.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "Python tutorial".into(),
                url: "https://b.com".into(),
                content: Some("c".into()),
                score: Some(0.6),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("b.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 2); // different titles kept
    }

    #[test]
    fn semantic_dedup_below_threshold() {
        use super::WebSearchResult;
        // Similar but below 0.8 Jaccard threshold
        let mut results = vec![
            WebSearchResult {
                title: "Rust is a systems programming language".into(),
                url: "https://a.com".into(),
                content: Some("c".into()),
                score: Some(0.5),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("a.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
            WebSearchResult {
                title: "Rust is a fast compiled language".into(),
                url: "https://b.com".into(),
                content: Some("c".into()),
                score: Some(0.6),
                highlights: None,
                raw_content: None,
                source: Some("ddg".into()),
                domain: Some("b.com".into()),
                published_date: None,
                favicon: None,
                chunks: None,
            },
        ];
        semantic_dedup(&mut results);
        assert_eq!(results.len(), 2); // below 0.8 threshold, both kept
    }

    // ─── compute_relevance_score tests ──────────────────────────

    #[test]
    fn compute_relevance_score_title_match_high() {
        use super::WebSearchResult;
        let result = WebSearchResult {
            title: "Rust programming language".into(),
            url: "https://rust-lang.org".into(),
            content: Some("Rust is a systems programming language.".into()),
            score: None,
            highlights: None,
            raw_content: None,
            source: Some("wikipedia".into()),
            domain: Some("wikipedia.org".into()),
            published_date: None,
            favicon: None,
            chunks: None,
        };
        let score = compute_relevance_score(&result, "rust programming");
        assert!(
            score > 0.6,
            "Expected high score for title match, got {}",
            score
        );
    }

    #[test]
    fn compute_relevance_score_no_match_low() {
        use super::WebSearchResult;
        let result = WebSearchResult {
            title: "Cooking recipes".into(),
            url: "https://cooking.com".into(),
            content: Some("Delicious recipes for dinner.".into()),
            score: None,
            highlights: None,
            raw_content: None,
            source: Some("ddg".into()),
            domain: Some("cooking.com".into()),
            published_date: None,
            favicon: None,
            chunks: None,
        };
        let score = compute_relevance_score(&result, "rust programming");
        assert!(
            score < 0.5,
            "Expected low score for no match, got {}",
            score
        );
    }

    #[test]
    fn compute_relevance_score_freshness_boost() {
        use super::WebSearchResult;
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let result = WebSearchResult {
            title: "Rust update".into(),
            url: "https://example.com".into(),
            content: Some("Rust gets new features.".into()),
            score: None,
            highlights: None,
            raw_content: None,
            source: Some("ddg".into()),
            domain: Some("example.com".into()),
            published_date: Some(today),
            favicon: None,
            chunks: None,
        };
        let score = compute_relevance_score(&result, "rust");
        // Fresh content should get a boost
        assert!(score > 0.5, "Expected freshness boost, got {}", score);
    }

    #[test]
    fn compute_relevance_score_authority_boost() {
        use super::WebSearchResult;
        let result = WebSearchResult {
            title: "Rust docs".into(),
            url: "https://doc.rust-lang.org".into(),
            content: Some("Official Rust documentation.".into()),
            score: None,
            highlights: None,
            raw_content: None,
            source: Some("ddg".into()),
            domain: Some("docs.rs".into()),
            published_date: None,
            favicon: None,
            chunks: None,
        };
        let score = compute_relevance_score(&result, "rust docs");
        // docs.rs has high authority (0.85)
        assert!(score > 0.6, "Expected authority boost, got {}", score);
    }

    // ─── chunked content tests ─────────────────────────────────

    #[test]
    fn bm25_chunk_scoring_returns_top_k() {
        let chunks = vec![
            "Rust is a systems programming language focused on safety and performance.".into(),
            "Python is a popular general-purpose language used in data science.".into(),
            "Rust ownership prevents data races at compile time with zero cost.".into(),
        ];
        let scored = bm25_score_chunks(&chunks, "rust safety", 2);
        assert_eq!(scored.len(), 2);
        // The Python chunk (no query terms) should not be in top 2
        assert!(
            scored.iter().all(|c| !c.content.contains("Python")),
            "Python chunk should rank lower than Rust chunks"
        );
        assert!(scored[0].score > 0.0);
    }

    #[test]
    fn bm25_chunk_scoring_preserves_index() {
        let chunks = vec!["hello world".into(), "foo bar baz".into()];
        let scored = bm25_score_chunks(&chunks, "hello", 2);
        assert_eq!(scored.len(), 2);
        let hello_chunk = scored.iter().find(|c| c.content.contains("hello")).unwrap();
        let foo_chunk = scored.iter().find(|c| c.content.contains("foo")).unwrap();
        assert!(hello_chunk.score > foo_chunk.score);
        assert_eq!(hello_chunk.index, 0);
        assert_eq!(foo_chunk.index, 1);
    }

    #[test]
    fn bm25_single_chunk() {
        let chunks = vec!["Only one paragraph here.".into()];
        let scored = bm25_score_chunks(&chunks, "anything", 5);
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].index, 0);
    }

    #[test]
    fn split_paragraphs_normal() {
        let text = "First paragraph with enough content to pass the filter and be included in results."
            .to_string()
            + "\n\n"
            + "Second paragraph also with enough content to survive the 40 char minimum filter."
            + "\n\n"
            + "Third paragraph of sufficient length that it should not be filtered out by the length check.";
        let paragraphs = super::split_paragraphs(&text);
        assert_eq!(paragraphs.len(), 3, "Expected 3 paragraphs after splitting");
        assert!(paragraphs[0].starts_with("First paragraph"));
        assert!(paragraphs[1].starts_with("Second paragraph"));
        assert!(paragraphs[2].starts_with("Third paragraph"));
    }

    #[test]
    fn split_paragraphs_filters_short() {
        let text = "A short line.\n\nAnother short one.";
        let paragraphs = super::split_paragraphs(text);
        assert!(
            paragraphs.is_empty(),
            "Short fragments should be filtered out"
        );
    }

    #[test]
    fn split_paragraphs_collapses_single_newlines() {
        let text = "Line one of paragraph.\nLine two of paragraph.\nLine three of paragraph.";
        let paragraphs = super::split_paragraphs(text);
        // No double newlines, so the whole text is one "paragraph"
        // but single newlines are collapsed into spaces
        assert_eq!(paragraphs.len(), 1);
        assert!(paragraphs[0].contains("Line one"));
        assert!(
            !paragraphs[0].contains('\n'),
            "Single newlines should be collapsed"
        );
    }
}
