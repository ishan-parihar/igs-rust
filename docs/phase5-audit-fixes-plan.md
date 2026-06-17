# IGS Phase 5: Audit-Driven Improvements Implementation Plan

> **For agentic workers:** Use subagent-driven-development or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all 12 audit findings to make IGS maximally consumable for AI agents — documented schemas, typed outputs, anti-purpose descriptions, complete test coverage, and clean docs.

**Architecture:** Four parallel workstreams: (1) P0 schema fixes in Rust types, (2) P1 docs/test fixes, (3) P2 code quality, (4) P3 test coverage. Each workstream is independent and can be parallelized.

**Tech Stack:** Rust (schemars, serde, rmcp), Python (pytest for integration tests), Markdown (docs)

---

## Workstream 1: P0 — Schema Documentation & Type Safety (CRITICAL)

### Task 1: Add doc comments to all undocumented input fields

**Files:**
- Modify: `src/tools/types.rs` — SourceUpsertInput, ResearchSearchInput, WebSearchInput, WebCrawlInput, RedditSearchInput, RedditFeedInput, PoolUpsertInput, PoolsUpsertInput
- Modify: `src/tools/types_base.rs` — fix stale intelligence.collect reference

**Estimated fields to document:** ~50 fields across 8 structs

- [ ] **Step 1: Document SourceUpsertInput (12 fields → 0 documented)**

```rust
/// Input for creating or updating a news source
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct SourceUpsertInput {
    /// Unique identifier for the source (e.g., "reuters", "bbc-news")
    pub id: String,
    /// Human-readable name of the source
    pub name: String,
    /// Base URL of the source (e.g., "https://www.reuters.com")
    pub url: String,
    /// Parser key to use for this source (e.g., "rss", "generic_html")
    pub parser: String,
    /// Pool IDs this source belongs to (e.g., ["GLOBAL_TECH_CYBER"])
    pub pools: Vec<String>,
    /// Country code(s) for the source (e.g., ["US", "GB"])
    #[serde(default)]
    pub countries: Vec<String>,
    /// City names for local sources (e.g., ["Delhi", "Mumbai"])
    #[serde(default)]
    pub cities: Vec<String>,
    /// Specific paths to fetch from the source (e.g., ["/technology", "/science"])
    #[serde(default)]
    pub paths: Vec<String>,
    /// Custom headers to send with requests (e.g., {"Authorization": "Bearer ..."})
    #[serde(default)]
    pub headers: Option<std::collections::HashMap<String, String>>,
    /// CSS selectors for generic_html parser (e.g., {"title": "h1.headline"})
    #[serde(default)]
    pub selectors: Option<std::collections::HashMap<String, String>>,
    /// Whether this source is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Tags for categorization (e.g., ["breaking", "tech"])
    #[serde(default)]
    pub tags: Vec<String>,
}
```

- [ ] **Step 2: Document ResearchSearchInput (6 fields → 0 documented)**

```rust
/// Input for searching academic papers
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct ResearchSearchInput {
    /// Search query string (e.g., "transformer architecture attention mechanism")
    pub query: String,
    /// Search engines to query (e.g., ["arxiv", "semantic_scholar"])
    #[serde(default)]
    pub sources: Vec<String>,
    /// Maximum number of results to return (default: 10)
    #[serde(default = "default_limit_10")]
    pub limit: usize,
    /// Filter by publication year (e.g., 2024)
    #[serde(default)]
    pub year: Option<u32>,
    /// Filter by author name (e.g., "Hinton")
    #[serde(default)]
    pub author: Option<String>,
    /// Sort results by relevance, date, or citations (default: "relevance")
    #[serde(default = "default_sort_relevance")]
    pub sort: String,
}
```

- [ ] **Step 3: Document WebSearchInput (9 fields → 1 documented)**

```rust
/// Input for web search using configured provider
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct WebSearchInput {
    /// Search query string
    pub query: String,
    /// Maximum number of results to return (default: 10, max: 20)
    #[serde(default = "default_max_results")]
    pub max_results: usize,
    /// Search type: "auto" (balanced), "fast" (quick), "deep" (comprehensive)
    #[serde(default = "default_search_type")]
    pub search_type: String,
    /// Include images in results (default: false)
    #[serde(default)]
    pub include_images: bool,
    /// Include raw HTML content (default: false)
    #[serde(default)]
    pub include_raw_content: bool,
    /// Domains to include (e.g., ["arxiv.org", "github.com"])
    #[serde(default)]
    pub include_domains: Vec<String>,
    /// Domains to exclude (e.g., ["reddit.com"])
    #[serde(default)]
    pub exclude_domains: Vec<String>,
    /// Country for localized results (e.g., "US", "IN")
    #[serde(default)]
    pub country: Option<String>,
    /// Time range filter: "day", "week", "month", "year"
    #[serde(default)]
    pub time_range: Option<String>,
}
```

- [ ] **Step 4: Document WebCrawlInput (10 fields → 2 documented)**

```rust
/// Input for BFS web crawling
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct WebCrawlInput {
    /// Starting URL for the crawl
    pub url: String,
    /// Maximum depth from starting URL (default: 2)
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    /// Maximum number of pages to crawl (default: 20)
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
    /// Respect robots.txt (default: true)
    #[serde(default = "default_true")]
    pub obey_robots: bool,
    /// Output format: "markdown" (default), "html", "semantic_tree"
    #[serde(default = "default_dump_format")]
    pub dump_format: String,
    /// When to capture page content: "load", "domcontentloaded", "networkidle", "done"
    #[serde(default = "default_wait_until")]
    pub wait_until: String,
    /// CSS selector to wait for before capturing
    #[serde(default)]
    pub wait_selector: Option<String>,
    /// Content stripping: "js", "css", "ui", "full"
    #[serde(default)]
    pub strip_mode: Option<String>,
    /// Include iframe content (default: false)
    #[serde(default)]
    pub include_frames: bool,
    /// URL path patterns to include (e.g., ["/docs/.*"])
    #[serde(default)]
    pub select_paths: Vec<String>,
}
```

- [ ] **Step 5: Document RedditSearchInput and RedditFeedInput**

```rust
/// Input for searching Reddit posts
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct RedditSearchInput {
    /// Search query string
    pub query: String,
    /// Subreddit to search in (e.g., "technology", "worldnews")
    #[serde(default)]
    pub subreddit: Option<String>,
    /// Sort results by: "relevance", "hot", "top", "new", "comments"
    #[serde(default = "default_sort_relevance")]
    pub sort: String,
    /// Time filter: "hour", "day", "week", "month", "year", "all"
    #[serde(default = "default_time_all")]
    pub time_filter: String,
    /// Maximum number of results (default: 10)
    #[serde(default = "default_limit_10")]
    pub limit: usize,
}

/// Input for fetching Reddit feed
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct RedditFeedInput {
    /// Subreddit to fetch from (e.g., "technology", "worldnews")
    pub subreddit: String,
    /// Feed type: "hot" (default), "new", "top", "rising"
    #[serde(default = "default_feed_type")]
    pub feed_type: String,
    /// Maximum number of posts to return (default: 25)
    #[serde(default = "default_limit_25")]
    pub limit: usize,
}
```

- [ ] **Step 6: Document PoolUpsertInput and fix types_base.rs stale reference**

```rust
/// Input for creating or updating a pool
#[derive(Deserialize, Serialize, JsonSchema, ToJsonSchema)]
pub struct PoolsUpsertInput {
    /// Unique identifier for the pool (e.g., "GLOBAL_TECH_CYBER")
    pub id: String,
    /// Human-readable name of the pool
    pub name: String,
    /// Description of what this pool contains
    pub description: String,
    /// Source IDs to include in this pool
    #[serde(default)]
    pub sources: Vec<String>,
}
```

In `src/tools/types_base.rs`, line 54 — replace:
```rust
/// Cache mode: "read_write" (default), "read_only", "write_only", "bypass"
```
With:
```rust
/// Cache mode: "read_write" (default), "read_only", "write_only", "bypass" (same as "bypass")
```

- [ ] **Step 7: Verify schema changes compile**

Run: `cargo check`
Expected: Clean compilation, no errors

Run: `cargo clippy --all-targets --all-features`
Expected: Zero warnings

---

### Task 2: Replace serde_json::Value with typed structs

**Files:**
- Modify: `src/tools/types.rs` — DiscoveryFilters.keywords, NewsEnrichOutput.items/meta, WebScrapeOutput.meta, ResearchDownloadOutput.metadata, LpToolOutput.meta
- Create: `src/tools/types.rs` — new typed structs (add before the structs that use them)

**Estimated new structs:** 5-6 small structs replacing 6 opaque Value fields

- [ ] **Step 1: Create typed struct for DiscoveryFilters.keywords**

Replace:
```rust
pub keywords: serde_json::Value,
```
With:
```rust
/// Keywords for filtering sources. Accepts multiple formats:
/// - Single string: "AI safety"
/// - Array of strings: ["AI", "safety"]
/// - Array of arrays (OR groups): [["AI", "safety"], ["machine learning"]]
pub keywords: KeywordFilter,
```

Add new type:
```rust
/// Flexible keyword filter that supports string, array, or nested array formats
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToJsonSchema)]
#[serde(untagged)]
pub enum KeywordFilter {
    /// Single keyword string
    Single(String),
    /// Array of keywords
    Multiple(Vec<String>),
    /// Nested arrays for OR-group logic (e.g., [["AI", "safety"], ["ML"]])
    Nested(Vec<Vec<String>>),
}
```

- [ ] **Step 2: Create typed struct for NewsEnrichOutput**

Replace:
```rust
pub items: Vec<serde_json::Value>,
pub meta: serde_json::Value,
```
With:
```rust
/// Enriched news items with NLP data
pub items: Vec<EnrichedItem>,
/// Metadata about the enrichment process
pub meta: EnrichmentMeta,
```

Add new types:
```rust
/// A single enriched news item with NLP annotations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToJsonSchema)]
pub struct EnrichedItem {
    /// Original news item fields
    #[serde(flatten)]
    pub item: serde_json::Value,
    /// Extracted topics from the content
    #[serde(default)]
    pub topics: Vec<String>,
    /// Named entities detected in the content
    #[serde(default)]
    pub entities: Vec<EntityInfo>,
    /// Sentiment analysis result
    #[serde(default)]
    pub sentiment: Option<SentimentResult>,
    /// Brief summary of the article
    #[serde(default)]
    pub summary: Option<String>,
}

/// Metadata about the enrichment process
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToJsonSchema)]
pub struct EnrichmentMeta {
    /// Number of items enriched
    pub enriched_count: usize,
    /// NLP features applied (e.g., ["topics", "entities", "sentiment", "summary"])
    pub features: Vec<String>,
}
```

- [ ] **Step 3: Create typed struct for WebScrapeOutput.meta**

Replace:
```rust
pub meta: serde_json::Value,
```
With:
```rust
/// Metadata about the scrape operation
pub meta: ScrapeMeta,
```

Add new type:
```rust
/// Metadata about a web scrape operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToJsonSchema)]
pub struct ScrapeMeta {
    /// Final URL after redirects
    pub url: String,
    /// HTTP status code
    pub status: u16,
    /// Content type of the response
    pub content_type: Option<String>,
    /// Time taken in milliseconds
    pub elapsed_ms: u64,
    /// Whether JavaScript was rendered (Lightpanda mode)
    pub js_rendered: bool,
}
```

- [ ] **Step 4: Create typed struct for ResearchDownloadOutput.metadata**

Replace:
```rust
pub metadata: serde_json::Value,
```
With:
```rust
/// Paper metadata
pub metadata: PaperMetadata,
```

Add new type:
```rust
/// Metadata about a downloaded research paper
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToJsonSchema)]
pub struct PaperMetadata {
    /// Paper title
    pub title: String,
    /// Paper ID (e.g., "arxiv:2301.00001")
    pub id: String,
    /// Publication year
    pub year: Option<u32>,
    /// Number of pages
    pub pages: Option<u32>,
    /// File size in bytes
    pub file_size: u64,
    /// Local file path where PDF was saved
    pub file_path: String,
}
```

- [ ] **Step 5: Create typed struct for LpToolOutput.meta**

Replace:
```rust
pub meta: serde_json::Value,
```
With:
```rust
/// Metadata about the browser operation
pub meta: BrowserMeta,
```

Add new type:
```rust
/// Metadata about a Lightpanda browser operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToJsonSchema)]
pub struct BrowserMeta {
    /// Current URL after operation
    pub url: String,
    /// Page title
    pub title: Option<String>,
    /// Operation type (e.g., "goto", "click", "fill")
    pub operation: String,
    /// Time taken in milliseconds
    pub elapsed_ms: u64,
}
```

- [ ] **Step 6: Update server.rs to construct typed structs**

Update all places where `serde_json::Value` was constructed for these fields to use the new typed structs instead. Search for patterns like `serde_json::json!({"url": ...})` in server.rs and replace with struct construction.

- [ ] **Step 7: Verify type changes compile**

Run: `cargo check`
Expected: Clean compilation, no errors

Run: `cargo clippy --all-targets --all-features`
Expected: Zero warnings

Run: `cargo test`
Expected: All tests pass

---

### Task 3: Add anti-purpose patterns to tool descriptions

**Files:**
- Modify: `src/server.rs` — all 41 tool descriptions (add "Do NOT use when..." lines)

**Guiding principle:** Each tool description should answer: What does it do? When to use it? When NOT to use it?

- [ ] **Step 1: Add anti-purpose to discovery tools**

Example pattern for `pools.list`:
```
description: "List all available source pools. Returns pool IDs and metadata. Use to discover what news categories are available. Do NOT use to fetch actual news content — use news.fetch instead."
```

Apply similar anti-purpose lines to: `pools.upsert`, `pools.delete`, `sources.list`, `sources.upsert`, `sources.delete`, `sources.autodiscover`, `sources.enableGenericScraper`, `sources.countries`, `sources.cities`, `sources.domains`, `parsers.list`

- [ ] **Step 2: Add anti-purpose to news tools**

Example pattern for `news.fetch`:
```
Do NOT use for: web scraping (use web.scrape), academic papers (use research.*), Reddit posts (use reddit.*).
```

Apply to: `news.fetch`, `news.testSource`, `news.enrich`

- [ ] **Step 3: Add anti-purpose to research tools**

Example pattern for `research.search`:
```
Do NOT use for: general web search (use web.search), news articles (use news.fetch), Reddit discussions (use reddit.*).
```

Apply to: `research.search`, `research.paper`, `research.download`

- [ ] **Step 4: Add anti-purpose to web tools**

Example pattern for `web.search`:
```
Do NOT use for: academic papers (use research.search), news articles (use news.fetch), Reddit posts (use reddit.*).
```

Apply to: `web.search`, `web.scrape`, `web.crawl`, `web.map`

- [ ] **Step 5: Add anti-purpose to insights tools**

Example pattern for `insights.findConnections`:
```
Do NOT use for: fetching news (use news.fetch), web search (use web.search), paper research (use research.*).
```

Apply to: `insights.findConnections`, `insights.trendingEntities`, `insights.indexArticles`, `insights.getStats`, `insights.clearIndex`

- [ ] **Step 6: Add anti-purpose to social tools**

Example pattern for `reddit.search`:
```
Do NOT use for: general web search (use web.search), news articles (use news.fetch), academic papers (use research.*).
```

Apply to: `reddit.search`, `reddit.feed`

- [ ] **Step 7: Add anti-purpose to browser tools**

Example pattern for `lightpanda.goto`:
```
Do NOT use for: simple HTTP fetching (use web.scrape), API calls, or non-web content.
```

Apply to all 12 `lightpanda.*` tools.

- [ ] **Step 8: Verify descriptions compile**

Run: `cargo check`
Expected: Clean compilation

Run: `cargo test`
Expected: All tests pass

---

## Workstream 2: P1 — Documentation & Test Fixes

### Task 4: Fix README.md inaccuracies

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Fix tool count from 42 to 41**

Change line 6 from `42 tools` to `41 tools`
Change line 10 from `42 (30 core + 12 Lightpanda)` to `41 (29 core + 12 Lightpanda)`

- [ ] **Step 2: Update download URL from v0.3.0 to v0.4.0**

Find the v0.3.0 download reference and update to v0.4.0

- [ ] **Step 3: Add missing reddit.feed to tool table**

Add `reddit.feed` to the Social group tool table alongside `reddit.search`

- [ ] **Step 4: Remove phantom references to deprecated tools**

Remove any mentions of `intelligence.collect` and `insights.findAllConnections` from documentation

---

### Task 5: Fix AGENTS.md inaccuracies

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Add reddit.feed to tool lists**

Add `reddit.feed` to the Social group in both the main table and quick reference table

- [ ] **Step 2: Update Social group tool count**

Change from `reddit.search` only to `reddit.search`, `reddit.feed`

---

### Task 6: Fix registry.rs tool count comment

**Files:**
- Modify: `src/tools/registry.rs`

- [ ] **Step 1: Update tool count comment**

Change line 2 comment from "43" to "41" tools

---

### Task 7: Clean up test_all_tools.py dead calls

**Files:**
- Modify: `tests/test_all_tools.py`

- [ ] **Step 1: Remove dead insights.findAllConnections test**

Find and delete the test that calls the removed `insights.findAllConnections` tool

- [ ] **Step 2: Remove dead intelligence.collect test**

Find and delete the test that calls the removed `intelligence.collect` tool

- [ ] **Step 3: Update any references to removed tools**

Search for any other mentions and update to use current tool names

- [ ] **Step 4: Run tests to verify cleanup**

Run: `cd tests && python3 -m pytest test_all_tools.py -v`
Expected: Tests pass (minus expected external service failures)

---

## Workstream 3: P2 — Code Quality

### Task 8: Create HasFormat macro to reduce boilerplate

**Files:**
- Create: `src/tools/types.rs` — add macro definition
- Modify: `src/tools/types.rs` — replace 15 impl blocks with macro invocations

- [ ] **Step 1: Define the macro**

```rust
/// Macro to implement HasFormat for a type with a single `format` field
macro_rules! impl_has_format {
    ($type:ty) => {
        impl HasFormat for $type {
            fn format(&self) -> OutputFormat {
                self.format.clone()
            }
        }
    };
}
```

- [ ] **Step 2: Replace all 15 HasFormat impl blocks with macro calls**

Replace each:
```rust
impl HasFormat for SomeType {
    fn format(&self) -> OutputFormat {
        self.format.clone()
    }
}
```
With:
```rust
impl_has_format!(SomeType);
```

- [ ] **Step 3: Verify macro compiles**

Run: `cargo check`
Expected: Clean compilation

Run: `cargo clippy --all-targets --all-features`
Expected: Zero warnings

---

### Task 9: Remove redundant #[serde(default)] on Option fields

**Files:**
- Modify: `src/tools/types_base.rs` — 14 instances

- [ ] **Step 1: Identify all Option fields with redundant defaults**

Search for `Option<` fields that have `#[serde(default)]` — since `Option` already defaults to `None`, the explicit default is redundant.

- [ ] **Step 2: Remove redundant defaults**

Remove `#[serde(default)]` from all `Option<T>` fields (keep it on non-Option fields).

- [ ] **Step 3: Verify removal compiles**

Run: `cargo check`
Expected: Clean compilation

---

## Workstream 4: P3 — Test Coverage

### Task 10: Add tools/list verification test

**Files:**
- Modify: `tests/test_all_tools.py`

- [ ] **Step 1: Add tools/list test**

```python
def test_tools_list():
    """Verify all expected tools are registered"""
    result = call_tool("tools/list", {})
    assert result is not None
    tool_names = [t["name"] for t in result.get("tools", [])]
    
    expected_tools = [
        "pools.list", "pools.upsert", "pools.delete",
        "sources.list", "sources.upsert", "sources.delete",
        "sources.autodiscover", "sources.enableGenericScraper",
        "sources.countries", "sources.cities", "sources.domains",
        "parsers.list",
        "news.fetch", "news.testSource", "news.enrich",
        "research.search", "research.paper", "research.download",
        "web.search", "web.scrape", "web.crawl", "web.map",
        "insights.findConnections", "insights.trendingEntities",
        "insights.indexArticles", "insights.getStats", "insights.clearIndex",
        "reddit.search", "reddit.feed",
        "lightpanda.goto", "lightpanda.markdown", "lightpanda.links",
        "lightpanda.evaluate", "lightpanda.semantic_tree", "lightpanda.structuredData",
        "lightpanda.detectForms", "lightpanda.click", "lightpanda.fill",
        "lightpanda.scroll", "lightpanda.waitForSelector", "lightpanda.interactiveElements",
    ]
    
    for tool in expected_tools:
        assert tool in tool_names, f"Missing tool: {tool}"
```

- [ ] **Step 2: Run verification test**

Run: `cd tests && python3 -m pytest test_all_tools.py::test_tools_list -v`
Expected: PASS

---

### Task 11: Add missing tool tests for untested tools

**Files:**
- Modify: `tests/test_all_tools.py`

- [ ] **Step 1: Add reddit.feed test**

```python
def test_reddit_feed():
    result = call_tool("reddit.feed", {"subreddit": "technology", "limit": 5})
    assert result is not None
    data = json.loads(result) if isinstance(result, str) else result
    assert "posts" in data or "items" in data
```

- [ ] **Step 2: Add research.paper test**

```python
def test_research_paper():
    result = call_tool("research.paper", {"id": "arxiv:2301.00001"})
    assert result is not None
    data = json.loads(result) if isinstance(result, str) else result
    assert "title" in data or "error" in data
```

- [ ] **Step 3: Add web.crawl test (requires Lightpanda)**

```python
@pytest.mark.skipif(not os.getenv("LIGHTPANDA_ENABLED"), reason="Lightpanda not enabled")
def test_web_crawl():
    result = call_tool("web.crawl", {"url": "https://example.com", "max_depth": 1, "max_pages": 2})
    assert result is not None
```

- [ ] **Step 4: Add sources.upsert and sources.delete test**

```python
def test_sources_upsert_and_delete():
    # Create a test source
    result = call_tool("sources.upsert", {
        "id": "test_source_temp",
        "name": "Test Source",
        "url": "https://example.com/feed",
        "parser": "rss",
        "pools": ["GLOBAL_TECH_CYBER"],
    })
    assert result is not None
    
    # Delete the test source
    result = call_tool("sources.delete", {"id": "test_source_temp"})
    assert result is not None
```

- [ ] **Step 5: Run all new tests**

Run: `cd tests && python3 -m pytest test_all_tools.py -v -k "reddit_feed or research_paper or web_crawl or sources_upsert"`
Expected: Tests pass (excluding skipped Lightpanda tests)

---

## Execution Order & Parallelization

### Phase 1 (Parallel — can run simultaneously)
- Task 1: Doc comments on input fields
- Task 2: Typed structs for serde_json::Value
- Task 4: Fix README.md
- Task 5: Fix AGENTS.md
- Task 6: Fix registry.rs comment
- Task 7: Clean test_all_tools.py dead calls

### Phase 2 (After Phase 1)
- Task 3: Anti-purpose patterns (needs server.rs open)
- Task 8: HasFormat macro (needs types.rs from Task 1/2)
- Task 9: Remove redundant defaults (needs types_base.rs from Task 1)

### Phase 3 (After Phase 2)
- Task 10: tools/list verification test
- Task 11: Missing tool tests

### Final Verification
- `cargo check` + `cargo clippy` + `cargo test` (61/61)
- `python3 -m pytest test_all_tools.py -v` (all non-external tests pass)
- Manual smoke test of IGS MCP server

---

## Commit Strategy

1. **Commit 1:** "docs(types): add doc comments to all undocumented input fields"
2. **Commit 2:** "feat(types): replace serde_json::Value with typed structs"
3. **Commit 3:** "docs(server): add anti-purpose patterns to all tool descriptions"
4. **Commit 4:** "docs: fix README.md and AGENTS.md inaccuracies"
5. **Commit 5:** "refactor(types): extract HasFormat macro to reduce boilerplate"
6. **Commit 6:** "chore(types): remove redundant serde(default) on Option fields"
7. **Commit 7:** "test: add tools/list verification and missing tool tests"
8. **Commit 8:** "fix(tests): remove dead test calls to deprecated tools"

---

## Success Criteria

- [ ] All 41 tool input fields have `///` doc comments
- [ ] Zero `serde_json::Value` fields in tool types (except where truly dynamic)
- [ ] All 41 tool descriptions include anti-purpose patterns
- [ ] README.md accurate: 41 tools, correct version, all tools listed
- [ ] AGENTS.md accurate: includes reddit.feed, correct counts
- [ ] test_all_tools.py: no dead calls, all tools have at least one test
- [ ] HasFormat macro replaces 15 duplicate impl blocks
- [ ] No redundant `#[serde(default)]` on Option fields
- [ ] `cargo check` clean, `cargo clippy` zero warnings, `cargo test` 61/61
- [ ] `python3 -m pytest` passes (excluding expected external failures)
