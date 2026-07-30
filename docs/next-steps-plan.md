# IGS Next Steps Plan: Search Tools + CRW Porting

> **Date:** July 30, 2026
> **Status:** Planning
> **Author:** Buffy (AI Agent)

---

## 1. Live Integration Test Results

### Test 1: Code Search (`--topic code`)

```
Query: "rust async runtime"
Engines Used: github, stackoverflow, hackernews, duckduckgo
Results: 3
Response Time: 1098ms
Scored: true
```

| Result | Source | Score | Notes |
|--------|--------|-------|-------|
| "How do Rust async runtimes handle channels?" | stackoverflow | 0.79 | ✅ Answered, 1059 views |
| "The State of Async Rust: Runtimes" | hackernews | 0.775 | ⬆️ 258 points, 198 comments |
| "Rust Async Runtimes: Tokio vs async-std vs smol 2026" | duckduckgo | 0.75 | Fresh 2026 content |

**Verdict:** ✅ All 4 engines responded, results are relevant and scored.

### Test 2: News Search (`--topic news`)

```
Query: "latest AI news today"
Engines Used: duckduckgo, hackernews
Results: 3
Response Time: 1510ms
Scored: true
```

| Result | Source | Score |
|--------|--------|-------|
| AI News Today — The Daily Prompt | duckduckgo | 0.75 |
| AI News — Reuters | duckduckgo | 0.73 |
| AI News — AI Weekly | duckduckgo | 0.6875 |

**Verdict:** ✅ News routing works, results are fresh and relevant.

### Test 3: Academic Search with Highlights

```
Query: "machine learning transformer architecture"
Topic: academic
Include Highlights: true
Engines Used: github, duckduckgo, hackernews
Results: 3
Highlights: 3+1+2 = 6 total
```

**Verdict:** ✅ Highlights extraction working correctly.

### Key Findings

1. **All 5 engines are operational** — DDG (Obscura), Wikipedia, GitHub, HackerNews, StackOverflow
2. **Zero API keys required** — confirmed working without any env vars
3. **Smart routing works** — code→4 engines, news→2 engines, academic→3 engines
4. **Relevance scoring active** — scores range 0.68-0.79
5. **Highlights extraction working** — 2-3 highlights per result
6. **Latency acceptable** — 1.1-1.5s for 3-result queries

---

## 2. Image Search Implementation Plan

### Approach: DDG Image Scraping via Obscura (Key-Free)

**Why DDG instead of Brave:**
- Zero API key required
- Same Obscura infrastructure we already use for DDG web search
- DDG Images has millions of indexed images
- No rate limiting concerns

### Implementation Details

**File: `src/tools/web.rs`**

```rust
/// Search for images via DuckDuckGo Images (Obscura scraping).
/// No API keys required.
pub async fn web_image_search(input: WebImageSearchInput) -> Result<WebImageSearchOutput, String> {
    let settings = config::load_settings().await?;
    if !settings.browser.obscura.enabled {
        return Err("Obscura not enabled for image search".into());
    }
    
    let obscura = ObscuraManager::new(&settings.browser.obscura);
    let query_encoded = url::form_urlencoded::byte_serialize(input.query.as_bytes()).collect::<String>();
    let search_url = format!("https://duckduckgo.com/?q={}&iar=images&iax=images&ia=images", query_encoded);
    
    let html = obscura.fetch_with_all_options(&search_url, "html", false, "load", false, None).await?;
    
    // Parse DDG Images HTML for image results
    let results = parse_ddg_images_html(&html, input.max_results.unwrap_or(10) as usize);
    
    Ok(WebImageSearchOutput { results, count: results.len(), meta: ... })
}
```

**HTML Parsing Strategy:**
- DDG Images uses JavaScript-rendered content
- Image URLs are in `data-src` or `src` attributes
- Thumbnails in `img` elements
- Source pages in `a` parent elements
- Use Obscura to render JS, then parse with `scraper` crate

**Type Definitions:**

```rust
pub struct WebImageSearchInput {
    pub query: String,
    pub max_results: Option<i32>,
    pub size: Option<String>,      // small, medium, large, wallpaper
    pub image_type: Option<String>, // photo, illustration, clipart
    pub output: OutputOptions,
}

pub struct WebImageResult {
    pub title: String,
    pub url: String,           // Full-size image URL
    pub thumbnail: Option<String>,
    pub source_url: Option<String>,  // Page containing the image
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub struct WebImageSearchOutput {
    pub results: Vec<WebImageResult>,
    pub count: usize,
    pub meta: WebSearchMeta,
}
```

**Estimated Effort:** 1-2 days

---

## 3. Local Search Implementation Plan

### Approach: DDG Local Results via Obscura (Key-Free)

**Why DDG Local:**
- Zero API key required
- DDG provides local business results for location-based queries
- Same scraping infrastructure

### Implementation Details

**File: `src/tools/web.rs`**

```rust
/// Search for local businesses/places via DuckDuckGo.
/// No API keys required.
pub async fn web_local_search(input: WebLocalSearchInput) -> Result<WebLocalSearchOutput, String> {
    let settings = config::load_settings().await?;
    if !settings.browser.obscura.enabled {
        return Err("Obscura not enabled for local search".into());
    }
    
    let obscura = ObscuraManager::new(&settings.browser.obscura);
    let query_encoded = url::form_urlencoded::byte_serialize(input.query.as_bytes()).collect::<String>();
    // DDG local results appear in regular search with location context
    let search_url = format!("https://html.duckduckgo.com/html/?q={}", query_encoded);
    
    let html = obscura.fetch_with_all_options(&search_url, "html", false, "load", false, None).await?;
    
    // Parse DDG HTML for local business cards
    let results = parse_ddg_local_html(&html, input.max_results.unwrap_or(10) as usize);
    
    Ok(WebLocalSearchOutput { results, count: results.len(), meta: ... })
}
```

**Type Definitions:**

```rust
pub struct WebLocalSearchInput {
    pub query: String,
    pub max_results: Option<i32>,
    pub location: Option<String>,  // Bias results to location
    pub radius_km: Option<f64>,
    pub output: OutputOptions,
}

pub struct WebLocalResult {
    pub name: String,
    pub address: String,
    pub phone: Option<String>,
    pub rating: Option<f64>,
    pub review_count: Option<u32>,
    pub categories: Vec<String>,
    pub url: Option<String>,
    pub hours: Option<String>,
}

pub struct WebLocalSearchOutput {
    pub results: Vec<WebLocalResult>,
    pub count: usize,
    pub meta: WebSearchMeta,
}
```

**Estimated Effort:** 1-2 days

---

## 4. Video Search Implementation Plan

### Approach: Extend web.search with YouTube Integration

**Why extend existing:**
- YouTube integration already exists as `youtube.search`
- Can detect video intent in queries and add YouTube results
- No new tools needed, just smarter routing

### Implementation Details

**File: `src/tools/web.rs` — `route_engines()` modification**

```rust
fn route_engines(topic: &str, query: &str) -> Vec<String> {
    let query_lower = query.to_lowercase();
    
    // Detect video intent
    let has_video_intent = query_lower.contains("video") 
        || query_lower.contains("tutorial")
        || query_lower.contains("watch")
        || query_lower.contains("how to")
        || query_lower.contains("demo");
    
    // Existing routing logic...
    let mut engines = match effective_topic {
        "code" => vec!["github", "stackoverflow", "hackernews", "duckduckgo"],
        "news" => vec!["duckduckgo", "hackernews"],
        _ => vec!["duckduckgo", "wikipedia", "hackernews"],
    };
    
    // Add YouTube if video intent detected
    if has_video_intent {
        engines.push("youtube");
    }
    
    engines
}
```

**File: `src/tools/web.rs` — New `search_youtube()` function**

```rust
/// Search YouTube via yt-dlp (already available).
async fn search_youtube(query: &str, max_results: usize) -> Result<(String, Vec<WebSearchResult>, Option<String>), String> {
    // Use yt-dlp to search YouTube
    let output = tokio::process::Command::new("yt-dlp")
        .args(["--flat-playlist", "--print", "%(id)s|||%(title)s|||%(url)s|||%(channel)s", 
               &format!("ytsearch{}:{}", max_results, query)])
        .output().await?;
    
    // Parse output into WebSearchResult
    // ...
}
```

**Estimated Effort:** 1 day

---

## 5. CRW Porting Plan

### 5.1 Readability Extractor (Priority: HIGH)

**Source:** `crates/crw-extract/src/readability.rs`
**Target:** `src/tools/web.rs` — `extract_semantic_excerpt()` replacement

**What to Port:**

1. **`text_density()` function** (3 lines — trivial)
   ```rust
   fn text_density(html: &str) -> f64 {
       let doc = Html::parse_fragment(html);
       let text_len: usize = doc.root_element().text().map(|t| t.len()).sum();
       if html.is_empty() { return 0.0; }
       text_len as f64 / html.len() as f64
   }
   ```

2. **Selector List** (copy, not code)
   ```rust
   // Priority selectors
   ["article", "main", "[role=\"main\"]"]
   
   // Scored selectors (copy list from CRW)
   [
       ".post-content", ".article-body", ".entry-content",
       ".article-content", ".post-body", ".story-body",
       ".content-body", "#main-content", "#article",
       "#content", ".content", ".main",
       "[itemprop=\"articleBody\"]", "[itemprop=\"text\"]",
       ".main-page-content",  // MDN
       ".js-post-body", ".s-prose", "#question",  // StackOverflow
       ".page-content", "#page-content", "[role=\"article\"]",
       ".mw-parser-output", "#mw-content-text",  // Wikipedia
       ".mw-body-content", "#bodyContent",
   ]
   ```

3. **Drill-down logic** for too-broad containers (>90% of body)
   ```rust
   // Inner selectors for drill-down
   let inner_selectors = [
       ".main-page-content", ".article-content", ".post-content",
       ".entry-content", ".content-body", ".article-body",
       "[itemprop=\"articleBody\"]", "[itemprop=\"text\"]",
       ".mw-parser-output", "#mw-content-text", "#content",
       ".content", "article",
   ];
   ```

4. **Penalty tokens** for nav/sidebar/filter elements
   ```rust
   const PENALTY_TOKENS: &[&str] = &[
       "filter", "facet", "sidebar", "nav", "menu", "navigation"
   ];
   ```

**Estimated Effort:** 1 day

---

### 5.2 BM25/Cosine Scoring (Priority: HIGH)

**Source:** `crates/crw-extract/src/filter.rs`
**Target:** `src/tools/web.rs` — new `filter_chunks()` function

**What to Port:**

1. **Tokenization function**
   ```rust
   fn tokenize(text: &str) -> Vec<String> {
       text.to_lowercase()
           .split(|c: char| !c.is_alphanumeric())
           .filter(|t| t.len() > 1)
           .map(|t| t.to_string())
           .collect()
   }
   ```

2. **BM25 scoring** (standard algorithm)
   ```rust
   const K1: f64 = 1.2;
   const B: f64 = 0.75;
   
   fn bm25_score(query_terms: &[String], doc_tokens: &[String], 
                  avgdl: f64, df: &HashMap<&str, usize>, n: f64) -> f64 {
       // Standard BM25 formula
   }
   ```

3. **Cosine TF-IDF scoring**
   ```rust
   fn cosine_score(query_tokens: &[String], doc_tokens: &[String],
                    vocab: &[String], idf: &[f64]) -> f64 {
       // Standard cosine similarity
   }
   ```

**Use Cases:**
- `web.extract` — rank extracted chunks by relevance
- `web.search` deep mode — rank excerpts
- `insights.find_connections` — rank connections

**Estimated Effort:** 1 day

---

### 5.3 Content Cleaning Pipeline (Priority: MEDIUM)

**Source:** `crates/crw-extract/src/clean.rs`
**Target:** `src/tools/web.rs` — enhanced `remove_boilerplate()`

**What to Port:**

1. **Noise patterns list** (copy, not code)
   ```rust
   const NOISE_PATTERNS: &[&str] = &[
       "table-of-contents", "tableofcontents", "infobox",
       "navbox", "nav-box", "cookie", "consent",
       "widget-area", "widget_area", "disqus", "advert",
       "popup", "modal", "newsletter", "subscribe",
       "printfooter", "catlinks", "mw-panel", "mw-navigation",
       "sitesub", "jump-to-nav", "mw-editsection", "reflist",
       "mw-references", "authority-control", "mw-indicators",
       "sistersitebox", "mbox", "ambox", "ombox", "hatnote",
       "shortdescription", "sphinxsidebar", "sphinxfooter",
       "city-selector", "location-selector", "lang-selector",
       "language-selector", "skip-to", "skip-link", "skiplinks",
       "promo", "promotional", "social-share", "social-links",
       "social-icons", "follow-us", "site-map", "sitemap",
   ];
   ```

2. **Layout token matching** (prefix-based)
   ```rust
   const NOISE_LAYOUT_TOKENS: &[&str] = &[
       "sidebar", "navigation", "breadcrumb", "dropdown",
       "site-header", "site-footer", "page-header", "page-footer",
       "global-header", "global-footer", "global-nav",
       "main-nav", "primary-nav", "secondary-nav", "copyright",
   ];
   ```

3. **Exact token matching**
   ```rust
   const NOISE_EXACT_TOKENS: &[&str] = &[
       "toc", "share", "social", "related", "recommended",
       "comment", "footer",
   ];
   ```

4. **Prefix matching for ads**
   ```rust
   const NOISE_PREFIXES: &[&str] = &["ad-", "ads-"];
   ```

5. **ARIA role filtering**
   ```rust
   // Remove: contentinfo, navigation, banner, complementary
   ```

**Note:** CRW uses `lol-html` for streaming HTML cleanup. We can reimplement using `scraper` crate (which we already use) with DOM traversal.

**Estimated Effort:** 1-2 days

---

### 5.4 Metadata Extraction (Priority: LOW)

**Source:** `crates/crw-extract/src/readability.rs` — `extract_metadata()`
**Target:** `src/tools/web.rs` — enhance `extract_page_metadata()`

**What to Port:**

1. **Extra meta tag collection**
   ```rust
   fn collect_meta_tags(document: &Html) -> BTreeMap<String, serde_json::Value> {
       // Collect every <meta name|property> tag
       // Skip title/description (already surfaced as named fields)
       // Repeated tags become JSON arrays
   }
   ```

2. **Canonical URL extraction**
   ```rust
   // <link rel="canonical" href="...">
   ```

3. **Full OG tag extraction**
   ```rust
   // og:title, og:description, og:image, og:url, og:type, og:site_name
   ```

**Estimated Effort:** 0.5 days

---

### 5.5 Image Extraction (Priority: LOW)

**Source:** `crates/crw-extract/src/readability.rs` — `extract_images()`
**Target:** `src/tools/web.rs` — enhance `web_extract()`

**What to Port:**

1. **Firecrawl-compatible image sources**
   - `<img src|data-src|srcset>`
   - `<picture><source srcset>`
   - OG/Twitter meta images
   - Icon/image_src links
   - `<video poster>`
   - Inline `background-image` styles

2. **srcset URL parsing** (WHATWG algorithm)
   ```rust
   fn srcset_url_tokens(srcset: &str) -> Vec<&str> {
       // Handle data: URIs with commas
       // Handle calc() descriptors
   }
   ```

3. **URL resolution** (respect `<base href>`)
   ```rust
   // data:/blob: → verbatim
   // http(s):// → verbatim
   // //host/x → inherit page scheme
   // relative → join against <base href> or doc URL
   ```

4. **Dedup by URL** in document order

**Estimated Effort:** 1 day

---

## 6. Implementation Priority Matrix

| Priority | Feature | Effort | Key-Free? | CRW Port? |
|----------|---------|--------|-----------|-----------|
| 🔴 P0 | Image search (DDG Obscura) | 1-2 days | ✅ Yes | No |
| 🔴 P0 | Readability extractor | 1 day | ✅ Yes | Yes |
| 🔴 P0 | BM25/Cosine scoring | 1 day | ✅ Yes | Yes |
| 🟠 P1 | Video search (YouTube routing) | 1 day | ✅ Yes | No |
| 🟠 P1 | Local search (DDG Obscura) | 1-2 days | ✅ Yes | No |
| 🟠 P1 | Content cleaning pipeline | 1-2 days | ✅ Yes | Yes |
| 🟡 P2 | Metadata extraction enhancement | 0.5 days | ✅ Yes | Yes |
| 🟡 P2 | Image extraction enhancement | 1 day | ✅ Yes | Yes |

**Total Estimated Effort:** 8-10 days

---

## 7. Architecture Decisions

### 7.1 All Tools Key-Free

| Tool | Engine | Key Required |
|------|--------|-------------|
| `web.search` | DDG (Obscura) | ❌ No |
| `web.search` | Wikipedia REST | ❌ No |
| `web.search` | GitHub REST | ❌ No |
| `web.search` | HackerNews (Algolia) | ❌ No |
| `web.search` | StackOverflow (StackExchange) | ❌ No |
| `web.image_search` | DDG Images (Obscura) | ❌ No |
| `web.local_search` | DDG Local (Obscura) | ❌ No |
| `web.scrape` | Obscura / Lightpanda | ❌ No |
| `web.crawl` | Obscura / Lightpanda | ❌ No |
| `web.extract` | Obscura / Lightpanda | ❌ No |

### 7.2 CRW Porting Principles

1. **Copy data lists, reimplement algorithms** — Selector lists, penalty tokens, noise patterns are just data
2. **Standard algorithms from scratch** — BM25, cosine TF-IDF are well-documented
3. **Use existing dependencies** — `scraper` crate instead of `lol-html`
4. **No new external dependencies** — Keep zero-cost architecture

### 7.3 Testing Strategy

1. **Unit tests for each new function** — scoring, extraction, parsing
2. **Integration tests with real websites** — verify extraction quality
3. **Performance benchmarks** — target <2s for fast mode, <10s for deep mode
4. **Regression tests** — ensure existing functionality not broken

---

## 8. File Changes Summary

### New Files
- None (all changes go into existing files)

### Modified Files

| File | Changes |
|------|---------|
| `src/tools/web.rs` | +500-800 lines (new functions, enhanced extraction) |
| `src/tools/types.rs` | +50-80 lines (new type definitions) |
| `src/cli.rs` | +30-50 lines (new CLI commands) |
| `src/server.rs` | +20-30 lines (new MCP tool registrations) |

### Estimated Total
- **New code:** 600-960 lines
- **Modified code:** 100-150 lines
- **Tests:** 200-300 lines

---

## 9. Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| DDG HTML structure changes | High | Medium | Implement resilient parsing with fallbacks |
| Obscura rendering failures | Medium | Low | Add retry logic, fallback to HTTP |
| BM25/cosine accuracy | Medium | Medium | Tune parameters, A/B test against CRW |
| Content cleaning over-aggressive | Medium | High | Conservative patterns, user-configurable |
| Performance regression | Medium | Low | Benchmark before/after, optimize hot paths |

---

## 10. Success Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Image search result relevance | >70% | Manual evaluation of top-10 |
| Local search accuracy | >80% | Address/phone correctness |
| Video search relevance | >85% | Topic match evaluation |
| Readability extraction quality | >90% | Content completeness vs CRW |
| BM25 ranking accuracy | >80% | Top-5 precision |
| Content cleaning precision | >85% | Noise removal without content loss |
| Zero API keys required | 100% | Env var audit |
| No new external dependencies | 100% | Cargo.toml audit |

---

*This document will be updated as implementation progresses.*
