# IGS Web Tools Upgrade Plan

> **Document Version:** 1.0  
> **Date:** July 30, 2026  
> **Status:** Planning  
> **Author:** Buffy (AI Agent)

---

## 1. Executive Summary

This document formalizes the upgrade roadmap for the IGS (Intelligence Gathering System) web tools layer. The current implementation provides functional but shallow web search, scrape, crawl, extract, and map capabilities. The goal is to bring these tools to **parity with proprietary alternatives** (Tavily, Firecrawl) while maintaining the zero-cost, self-contained architecture.

### Key Objectives
1. Match Tavily's search quality (relevance scoring, content depth, answer synthesis)
2. Match Firecrawl's extraction capabilities (structured JSON, content cleaning, screenshots)
3. Add missing high-value search engines (Hacker News, Stack Overflow)
4. Optimize output for AI-agent workflows (token efficiency, provenance, actionability)
5. Maintain zero external API cost for core functionality

---

## 2. Current State Assessment

### 2.1 Web Tools Inventory

| Tool | Function | Provider | Quality Rating |
|------|----------|----------|----------------|
| `web.search` | Multi-engine web search | DDG (Obscura), Brave API, Wikipedia REST, GitHub Search | ⚠️ 40% of Tavily |
| `web.scrape` | Single page scraping | HTTP (default), Obscura, Lightpanda | ⚠️ 50% of Firecrawl |
| `web.crawl` | BFS site crawling | Obscura, Lightpanda | ⚠️ 30% of Firecrawl |
| `web.extract` | Structured content extraction | Obscura + CSS selectors | ⚠️ 35% of Firecrawl |
| `web.map` | Sitemap URL discovery | Sitemap.xml parser | ⚠️ 20% of Firecrawl |

### 2.2 Social & Research Tools (Existing)

| Tool | Engine | Status | In `web.search`? |
|------|--------|--------|-------------------|
| `reddit.search` | Reddit JSON API (cookie auth) | ✅ Working | ❌ Not integrated |
| `reddit.feed` | Reddit JSON API | ✅ Working | ❌ Not integrated |
| `twitter.search` | Twitter/X GraphQL API (cookie auth) | ✅ Working | ❌ Not integrated |
| `twitter.read` | Twitter/X GraphQL API | ✅ Working | N/A |
| `youtube.search` | yt-dlp CLI | ✅ Working | ❌ Not integrated |
| `youtube.metadata` | yt-dlp CLI | ✅ Working | N/A |
| `youtube.subtitles` | yt-dlp CLI | ✅ Working | N/A |
| `research.search` | arXiv + Semantic Scholar | ✅ Working | ❌ Not integrated |
| `research.paper` | arXiv + Semantic Scholar | ✅ Working | N/A |
| `research.pubmed_search` | PubMed E-utilities | ✅ Working | ❌ Not integrated |

### 2.3 Missing Engines (Not Integrated)

| Engine | API Type | Free Tier | Rate Limit | Priority |
|--------|----------|-----------|------------|----------|
| **Hacker News** | Firebase REST + Algolia | Unlimited | No hard limit | 🔴 HIGH |
| **Stack Overflow** | StackExchange API | 10K/day | With API key | 🔴 HIGH |
| **DuckDuckGo News** | HTML scraping | Free | Unofficial | 🟡 MEDIUM |
| **Google Scholar** | No official API | N/A | N/A | ⚪ LOW (use Semantic Scholar instead) |

---

## 3. Gap Analysis: IGS vs Proprietary

### 3.1 vs Tavily API

| Feature | Tavily | IGS Current | Gap | Target |
|---------|--------|-------------|-----|--------|
| **Relevance scoring** | ML ranker (0.0-1.0) | None | ❌ Critical | Add lightweight scorer |
| **Content snippets** | AI-optimized 500-2000 chars | DDG snippets (~200 chars) | ❌ Major | Add content depth control |
| **Raw content in results** | Optional full page text | Only `depth=deep` mode | ⚠️ Moderate | Make opt-in per result |
| **Answer synthesis** | Built-in AI answer | DDG Instant Answer only | ❌ Major | Multi-source synthesis |
| **Research mode** | Multi-hop autonomous | Not available | ❌ Missing | Future enhancement |
| **Image search** | Returns image results | Not available | ❌ Missing | Future enhancement |
| **Structured output** | JSON schema enforcement | Basic JSON | ⚠️ Moderate | Add schema mode |
| **Highlight extraction** | Key sentence extraction | Not available | ❌ Missing | Add highlights field |
| **Favicon** | Included in results | Partial (Brave only) | ⚠️ Minor | Add to all engines |

### 3.2 vs Firecrawl API

| Feature | Firecrawl | IGS Current | Gap | Target |
|---------|-----------|-------------|-----|--------|
| **JSON extraction mode** | Pydantic/Zod schema | CSS selectors only | ❌ Critical | Add schema enforcement |
| **Screenshot capture** | Full page screenshots | Not available | ❌ Missing | Add via Obscura |
| **Interactive browsing** | `/interact` endpoint | browser.* (stateless) | ⚠️ Moderate | Per-call is acceptable |
| **Proxy rotation** | Built-in proxy pools | None | ❌ Missing | Future enhancement |
| **Persistent profiles** | Cookie/login persistence | None | ❌ Missing | Future enhancement |
| **Agent endpoint** | Autonomous URL discovery | Not available | ❌ Missing | Future enhancement |
| **Batch processing** | Multiple URLs in one call | Sequential | ⚠️ Moderate | Add batch mode |
| **Content cleaning** | Removes nav/footer/ads | Basic html-to-markdown | ⚠️ Moderate | Add cleaning pipeline |
| **Smart waiting** | Auto-detect dynamic content | Manual `wait_selector` | ⚠️ Minor | Add auto-wait |

---

## 4. Upgrade Roadmap

### Phase 1: Search Quality (Match Tavily Core)

**Goal:** Make `web.search` results as useful as Tavily's for AI agents.

#### 1.1 Relevance Scoring
- Add lightweight scorer combining:
  - **Keyword relevance** (title + content matching query terms): 50% weight
  - **Freshness** (newer = higher score): 20% weight  
  - **Source authority** (domain reputation lookup): 30% weight
- Apply scoring after dedup, before truncation
- Return scores in `WebSearchResult.score` field

```rust
fn compute_relevance_score(
    result: &WebSearchResult, 
    query: &str,
    domain_authorities: &HashMap<String, f64>,
) -> f64 {
    let keyword_score = keyword_relevance(&result.title, &result.content, query);
    let freshness_score = freshness_factor(result.published_date.as_deref());
    let authority_score = result.domain.as_ref()
        .and_then(|d| domain_authorities.get(d))
        .copied()
        .unwrap_or(0.5);
    
    keyword_score * 0.5 + freshness_score * 0.2 + authority_score * 0.3
}
```

#### 1.2 Content Depth Control
- Add `content_length` parameter to `WebSearchInput`:
  - `"minimal"`: Title + snippet only (fastest, ~100 chars)
  - `"standard"`: Title + 500 char excerpt (default)
  - `"full"`: Title + 2000 char excerpt (slowest, most context)
- Implement via `extract_semantic_excerpt` with configurable `max_chars`

#### 1.3 Highlight Extraction
- Add `highlights` field to `WebSearchResult`:
  ```rust
  pub highlights: Option<Vec<String>>,  // Key sentences matching query
  ```
- Extract using simple TF-IDF sentence scoring
- Limit to 3-5 highlights per result (token-efficient for LLM context)

#### 1.4 Answer Synthesis
- Combine outputs from multiple engines:
  - DuckDuckGo Instant Answer API
  - Brave AI summary
  - Wikipedia summary (first result if high relevance)
- Merge into unified `WebSearchOutput.answer`
- Add confidence score to answer

#### 1.5 Add Hacker News Engine
- Use Algolia HN Search API (free, no key required):
  ```
  https://hn.algolia.com/api/v1/search?query={query}&tags=story&hitsPerPage={limit}
  ```
- Parse: title, URL, points, author, comments, created_at
- Map to `WebSearchResult` with `source: "hackernews"`

#### 1.6 Add Stack Overflow Engine
- Use StackExchange API (free with API key):
  ```
  https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle={query}&site=stackoverflow
  ```
- Parse: title, link, score, answer_count, is_answered, tags
- Map to `WebSearchResult` with `source: "stackoverflow"`

**Estimated Effort:** 2-3 days  
**Files Modified:** `src/tools/web.rs`, `src/tools/types.rs`

---

### Phase 2: Extraction Quality (Match Firecrawl Core)

**Goal:** Make `web.extract` as powerful as Firecrawl's extraction.

#### 2.1 Structured JSON Extraction
- Add `output_schema` parameter to `WebExtractInput`:
  ```rust
  pub output_schema: Option<serde_json::Value>,  // JSON schema for extraction
  pub extract_prompt: Option<String>,  // Natural language extraction prompt
  ```
- Implement using CSS selectors + text analysis to match schema fields
- Return validated JSON conforming to schema

#### 2.2 Screenshot Capture
- Add `screenshot` parameter to `WebScrapeInput`:
  ```rust
  pub screenshot: Option<bool>,  // Capture full page screenshot
  pub screenshot_format: Option<String>,  // "png" or "jpeg"
  pub screenshot_quality: Option<u32>,  // 1-100 for jpeg
  ```
- Use Obscura's screenshot capability
- Return as base64-encoded string in output

#### 2.3 Content Cleaning Pipeline
- Add post-processing to remove:
  - Navigation bars, headers, footers
  - Cookie banners, consent dialogs
  - Advertisements, sponsored content
  - Social media widgets
  - Boilerplate text (copyright, privacy policy links)
- Use heuristics: element position, class names, content patterns
- Apply before markdown conversion

#### 2.4 Metadata Enrichment
- Auto-extract and return in `WebExtractOutput.metadata`:
  - Author name (from meta tags, schema.org)
  - Publish date (from meta tags, schema.org, URL patterns)
  - Word count, estimated reading time
  - Language detection
  - Content type (article, product, documentation, etc.)

#### 2.5 Batch Processing
- Extend `web.extract` to accept multiple URLs:
  ```rust
  pub struct WebExtractInput {
      pub url: Option<String>,  // Single URL (backward compat)
      pub urls: Option<Vec<String>>,  // Multiple URLs (new)
      // ... other fields
  }
  ```
- Process URLs in parallel with configurable concurrency limit
- Return array of extraction results

**Estimated Effort:** 2-3 days  
**Files Modified:** `src/tools/web.rs`, `src/tools/types.rs`

---

### Phase 3: Intelligence Features (Beyond Proprietary)

**Goal:** Add capabilities that exceed what Tavily/Firecrawl offer.

#### 3.1 Smart Engine Routing
- Analyze query intent to select optimal engines:
  - **Code queries** → GitHub + Stack Overflow + Hacker News
  - **Medical queries** → PubMed + Wikipedia
  - **News queries** → DuckDuckGo + Brave + Hacker News
  - **Academic queries** → Semantic Scholar + arXiv
  - **Product queries** → DuckDuckGo + Brave (with shopping filter)
  - **Local queries** → DuckDuckGo + Brave (with location context)
- Implement via keyword classification + heuristics

#### 3.2 Cross-Source Semantic Deduplication
- Current: URL-based dedup only
- Upgrade: Title + content similarity dedup
- Use Jaccard similarity on word sets
- Threshold: 0.8 similarity → consider duplicate
- Keep the version from higher-authority source

#### 3.3 Result Caching
- Cache search results with TTL:
  - News queries: 5 minutes
  - Evergreen queries: 24 hours
  - Code queries: 12 hours
- Use existing `HttpClient` cache infrastructure
- Return cached results with `cache_hit: true` metadata

#### 3.4 Streaming Mode
- Return results as they arrive from each engine
- Use SSE (Server-Sent Events) for MCP transport
- Allow agent to process first results while waiting for others

#### 3.5 Confidence Scoring
- Per-result confidence based on:
  - Source authority (known reliable sources get higher scores)
  - Content freshness (newer = higher confidence)
  - Result diversity (results from multiple engines = higher confidence)
  - Snippet quality (longer, more relevant snippets = higher confidence)

**Estimated Effort:** 3-4 days  
**Files Modified:** `src/tools/web.rs`, `src/tools/types.rs`, `src/server.rs`

---

### Phase 4: Advanced Capabilities (Future)

**Goal:** Implement advanced features for power users.

#### 4.1 Research Mode
- Multi-hop autonomous research:
  1. Initial search to understand topic
  2. Identify knowledge gaps
  3. Targeted follow-up searches
  4. Synthesize findings into comprehensive report
- Similar to Tavily's `/research` endpoint

#### 4.2 Image Search
- Add `web.image_search` tool
- Use Brave Image Search API (free tier available)
- Return image URLs, thumbnails, source pages

#### 4.3 Video Search
- Extend YouTube integration
- Add video search to `web.search` when query indicates video intent
- Return video metadata + transcript excerpts

#### 4.4 Local Search
- Add location-aware search
- Use Brave Local Search API
- Return businesses, addresses, reviews

**Estimated Effort:** 5-7 days  
**Files Modified:** New tool files, `src/tools/web.rs`, `src/tools/types.rs`

---

## 5. Implementation Priority Matrix

| Priority | Phase | Impact | Effort | Dependencies |
|----------|-------|--------|--------|--------------|
| 🔴 P0 | 3.1 Smart engine routing | High | **Low** | None |
| 🔴 P0 | 1.5 Hacker News engine | High | Low | None |
| 🔴 P0 | 1.6 Stack Overflow engine | High | Low | None |
| 🔴 P0 | 1.1 Relevance scoring (BM25/cosine from CRW) | High | Medium | None |
| 🟠 P1 | 1.2 Content depth control | Medium | Low | 1.1 |
| 🟠 P1 | 1.3 Highlight extraction | Medium | Medium | 1.1 |
| 🟠 P1 | 1.4 Answer synthesis | Medium | Medium | None |
| 🟡 P2 | 2.1 Structured JSON extraction | High | Medium | None |
| 🟡 P2 | 2.3 Content cleaning (readability from CRW) | Medium | Medium | Phase A |
| 🟡 P2 | 2.4 Metadata enrichment | Medium | Low | None |
| 🟢 P3 | 3.2 Semantic dedup | Low | Medium | None |
| 🟢 P3 | 3.3 Result caching | Low | Low | None |
| ⚪ P4 | 4.1 Research mode | High | High | All above |
| ⚪ P4 | 4.2 Image search | Medium | Medium | None |

> **Note:** Smart routing (3.1) moved to P0 — it's the highest-leverage, lowest-effort change. Reordering based on code review: new engines first, then scoring, then extraction.

---

## 6. Success Metrics

### 6.1 Quantitative Metrics

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| Search result relevance (manual eval) | ~40% | >80% | Top-5 precision@5 |
| Content snippet length | ~200 chars | 500-2000 chars | Average snippet length |
| Answer synthesis accuracy | N/A | >70% | Manual evaluation |
| Engine coverage | 4 engines | 6 engines | Count |
| Extraction schema compliance | 0% | >90% | Schema validation |

### 6.2 Qualitative Metrics

- **AI Agent UX:** Results should provide enough context for agents to complete tasks without additional lookups
- **Token Efficiency:** Highlights and summaries should reduce LLM context usage by 50%+
- **Provenance:** Every fact should be traceable to source URL with confidence score
- **Freshness:** News results should be <24 hours old, code results should reference current versions

---

## 7. Risk Assessment

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Rate limiting on free APIs | High | Medium | Implement caching, backoff, key rotation |
| Anti-bot blocking (Obscura) | High | Medium | Add proxy support, user-agent rotation |
| Schema extraction accuracy | Medium | High | Use LLM-assisted extraction as fallback |
| Breaking changes in APIs | Medium | Low | Pin API versions, implement adapters |
| Performance regression | Medium | Low | Benchmark before/after, A/B testing |

---

## 8. Dependencies

### 8.1 External Dependencies

| Dependency | Purpose | Cost | Status |
|------------|---------|------|--------|
| Algolia HN Search | Hacker News search | Free (unlimited) | ✅ Available |
| StackExchange API | Stack Overflow search | Free (10K/day) | ✅ Available |
| Brave Search API | Web search (already integrated) | Free (2K/month) | ✅ Integrated |
| Obscura | JS rendering, screenshots | Free (self-hosted) | ✅ Integrated |
| yt-dlp | YouTube data extraction | Free (CLI tool) | ✅ Integrated |

### 8.2 Internal Dependencies

- `HttpClient` (existing) — for all API calls
- `ObscuraManager` (existing) — for JS rendering, screenshots
- `scraper` crate (existing) — for HTML parsing
- `html-to-markdown-rs` (existing) — for content conversion

---

## 9. Testing Strategy

### 9.1 Unit Tests

- Relevance scoring function with known inputs
- Content extraction with various HTML structures
- Schema validation for structured extraction
- Engine routing logic

### 9.2 Integration Tests

- End-to-end search with each engine
- Scrape/Crawl with real websites
- Extract with various content types

### 9.3 Performance Tests

- Search latency (target: <2s for fast mode, <10s for deep mode)
- Scrape latency (target: <5s per page)
- Crawl throughput (target: >10 pages/minute)

### 9.4 Quality Tests

- Relevance scoring precision/recall
- Content cleaning accuracy
- Schema extraction compliance

---

## 10. Appendix

### 10.1 API Reference

#### Hacker News Algolia API
```
GET https://hn.algolia.com/api/v1/search
Parameters:
  - query: Search term
  - tags: story, comment, poll, etc.
  - hitsPerPage: Number of results (default 20)
  - page: Page number
  - numericFilters: created_at_i > {timestamp}
```

#### StackExchange API
```
GET https://api.stackexchange.com/2.3/search
Parameters:
  - order: desc, asc
  - sort: relevance, activity, votes, creation
  - intitle: Search in title
  - site: stackoverflow, serverfault, etc.
  - pagesize: Number of results (max 100)
  - filter: Output filter (optional)
```

### 10.2 Domain Authority List (Partial)

```rust
static DOMAIN_AUTHORITIES: &[(&str, f64)] = &[
    ("github.com", 0.95),
    ("stackoverflow.com", 0.90),
    ("arxiv.org", 0.85),
    ("wikipedia.org", 0.80),
    ("reddit.com", 0.75),
    ("news.ycombinator.com", 0.85),
    ("medium.com", 0.70),
    ("dev.to", 0.70),
    ("docs.python.org", 0.85),
    ("developer.mozilla.org", 0.90),
    // ... extend as needed
];
```

### 10.3 Content Cleaning Patterns

```rust
static CLEANING_SELECTORS: &[&str] = &[
    // Navigation
    "nav", "header", "footer", ".navbar", ".header", ".footer",
    // Cookie/consent
    ".cookie-banner", ".consent", "#cookie", ".gdpr",
    // Ads
    ".ad", ".advertisement", "[data-ad]", ".sponsored",
    // Social
    ".social-share", ".share-buttons", ".social-links",
    // Boilerplate
    ".copyright", ".legal", ".terms",
];
```

---

*This document is a living plan and will be updated as implementation progresses.*

---

## 11. CRW (fastCRW) Porting Analysis

> **Source:** https://github.com/us/crw (cloned July 30, 2026) to /home/ishanp/Documents/GitHub/MY-PROJECTS/MCP-AND-CLIS/crw

### 11.1 What CRW Is

CRW (fastCRW) is a Rust-based open-source web scraping API that is a Firecrawl alternative. It provides:
- Single-URL scrape → clean markdown/HTML
- Multi-page BFS crawl with robots.txt compliance
- URL discovery (map)
- Web search via SearXNG backend
- Structured JSON extraction via LLM
- BM25/cosine chunk scoring for RAG
- Firecrawl-compatible REST API

### 11.2 Portable Code Components

| Component | CRW Location | What It Does | Port Priority | Effort |
|-----------|-------------|--------------|---------------|--------|
| **Readability extractor** | `crates/crw-extract/src/readability.rs` | Text-density scoring across candidate selectors to pick the richest content element. Handles Wikipedia, MDN, StackOverflow, and generic sites. | 🔴 HIGH | Small |
| **BM25/Cosine chunk scoring** | `crates/crw-extract/src/filter.rs` | Filter and rank text chunks by relevance to a query. Returns scored chunks sorted by relevance. | 🔴 HIGH | Small |
| **Search reranking pipeline** | `crates/crw-search/src/rerank.rs` | Junk filtering (dictionary sites, bot checks, shopping), coverage gates, geo-competing detection, domain dedup. Proven to beat raw SearXNG scores. | 🟠 MEDIUM | Medium |
| **robots.txt parser** | `crates/crw-crawl/src/robots.rs` | RFC 9309-compliant robots.txt parser with wildcard matching, Allow/Disallow specificity, and query-string handling. | 🟡 LOW (we have Lightpanda/Obscura) | Small |
| **Content cleaning selectors** | `crates/crw-extract/src/readability.rs` | 25+ scored selectors for MDN, StackOverflow, Wikipedia, generic sites. Drill-down logic for too-broad containers. | 🔴 HIGH | Small |
| **Metadata extraction** | `crates/crw-extract/src/readability.rs` | Full metadata: title, description, OG tags, canonical URL, language, extra meta tags. | 🟠 MEDIUM | Small |
| **Image extraction** | `crates/crw-extract/src/readability.rs` | Firecrawl-compatible image extraction: img, picture, OG/Twitter meta, icons, video poster, background-image. srcset parsing. | 🟡 LOW | Medium |

### 11.3 What We Should NOT Port

| Component | Why Not |
|-----------|--------|
| **SearXNG client** | CRW uses SearXNG as a search backend. We already have our own multi-engine search (DDG, Brave, Wikipedia, GitHub). Adding SearXNG would be a dependency we don't need. |
| **LLM extraction pipeline** | CRW uses an external LLM for JSON schema extraction. We want to keep zero-LLM-dependency for core tools. |
| **Firecrawl API compatibility** | We don't need Firecrawl-compatible endpoints. |
| **Proxy rotation** | CRW has proxy support. We don't need this yet. |
| **PDF parsing** | CRW uses pdf-inspector. We already have unpdf for PDF-to-markdown. |

### 11.4 Recommended Porting Plan

**Phase A: Reimplement Readability Extractor (1 day)**
- Reimplement `text_density` (3 lines — trivial from scratch)
- Copy the **selector list** (not the code) from CRW's `readability.rs`:
  - Priority: `article`, `main`, `[role="main"]`
  - Scored: `.post-content`, `.article-body`, `.entry-content`, `.main-page-content`, `.js-post-body`, `.s-prose`, `#question`, `.mw-parser-output`, `#mw-content-text`, etc.
- Reimplement the drill-down logic for too-broad containers (>90% of body)
- Add penalty tokens for nav/sidebar/filter elements
- Apply to `web.extract` and `web.scrape` tools

**Phase B: Reimplement BM25/Cosine Scoring (1 day)**
- Reimplement BM25 using standard algorithm (Wikipedia reference)
- Reimplement cosine TF-IDF using standard algorithm
- Add to `web.search` for result ranking after dedup
- Use for highlight extraction (top-K relevant sentences)

**Phase C: Reimplement Reranking Pipeline (2 days)**
- Reimplement junk filtering using CRW's **data lists** (JUNK_HOSTS, STOPWORDS) — these are just string sets, not copyrightable
- Reimplement coverage gates (`MIN_COVERAGE = 0.5`)
- Reimplement domain dedup (`registrable` = last two labels of host)
- Apply after engine-specific scoring, before final output

**Phase D: Reimplement Content Cleaning (1 day)**
- Use the **selector list** from CRW (not the code)
- Add to `extract_semantic_excerpt` and `extract_main_text`
- Add the penalty tokens for nav/sidebar/filter elements

### 11.5 CRW vs IGS Architecture Comparison

| Aspect | CRW | IGS |
|--------|-----|-----|
| **Language** | Rust | Rust |
| **HTML parser** | scraper (same) | scraper (same) |
| **MD conversion** | Custom (lol-html) | html-to-markdown-rs |
| **Browser** | Lightpanda (optional) | Obscura + Lightpanda |
| **Search backend** | SearXNG (self-hosted) | Multi-engine (DDG, Brave, Wiki, GitHub) |
| **Extraction** | LLM-based (optional) | CSS selectors (no LLM) |
| **Scoring** | BM25 + cosine | None (to be added) |
| **Robots.txt** | Custom parser | Lightpanda/Obscura built-in |
| **API** | REST (Firecrawl-compatible) | MCP + CLI |
