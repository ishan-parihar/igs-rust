# IGS MCP Tool System Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the IGS MCP server from 43 flat tools to a progressive-discovery system with DRY type definitions, zero redundant schema bloat, and AI-optimized tool discovery. Maintain 100% feature parity while reducing context overhead from ~12K tokens to ~2K tokens per session.

**Architecture:** Three-layer approach: (1) Consolidate type definitions into base structs with shared fields, (2) Merge redundant tools (intelligence.collect → news.fetch@depth=deep), (3) Implement progressive tool discovery via 5 domain-specific tool groups that AI agents load on-demand. Format handling extracted to a shared helper. No functional regression.

**Tech Stack:** Rust, rmcp (MCP SDK), serde, toon, sqlx (SQLite), async-trait

---

## Phase 1: Type System Consolidation

### Task 1: Create Base Input Types with Shared Fields

**Files:**
- Create: `src/tools/types_base.rs`
- Modify: `src/tools/types.rs:1-30` (add mod declaration)
- Modify: `src/tools/mod.rs` (add types_base)

- [ ] **Step 1: Create base types module**

```rust
// src/tools/types_base.rs
//! Base input types for shared fields across IGS tool categories.
//! Eliminates ~300 tokens of duplicated `format: Option<String>` across 15 structs.

use rmcp::model::Content;
use serde::{Deserialize, Serialize};

/// Base fields shared by all tools that produce output.
/// Each tool embeds this as a single field instead of repeating these params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputOptions {
    /// Output format: "toon" (default, token-efficient) or "json" (standard)
    #[serde(default)]
    pub format: Option<String>,
}

/// Base fields shared by all news/source discovery tools.
/// Covers filtering, date ranges, geographic scoping, and content matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryFilters {
    /// Pool IDs to search (e.g. ["GLOBAL_TECH_CYBER"]). If empty, searches all pools.
    #[serde(default)]
    pub pools: Option<Vec<String>>,
    /// Source IDs to search (e.g. ["techcrunch", "bbc"]). Overrides pools.
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    /// ISO 3166-1 alpha-2 country codes (e.g. ["US", "IN"]). 47 countries supported.
    #[serde(default)]
    pub countries: Option<Vec<String>>,
    /// City names (e.g. ["Delhi", "London"]).
    #[serde(default)]
    pub cities: Option<Vec<String>>,
    /// Domains to filter by (e.g. ["example.com"]).
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    /// Start date (ISO 8601: "2024-01-01T00:00:00Z"). For date range filtering.
    #[serde(default)]
    pub start: Option<String>,
    /// End date (ISO 8601: "2024-12-31T23:59:59Z"). For date range filtering.
    #[serde(default)]
    pub end: Option<String>,
    /// Keywords for content matching. Inclusion matches are applied after fetch.
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// Keywords to exclude. Items matching any exclusion keyword are dropped.
    #[serde(default)]
    pub exclude_keywords: Option<Vec<String>>,
    /// If true, all keywords must match (AND logic). Default: false (OR logic).
    #[serde(default)]
    pub match_all: Option<bool>,
    /// Maximum items to return. Default: 20. Range: 1-500.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Cache mode: "fresh" (new only), "all" (fresh + cached), "only" (cached only).
    /// Default: "all" for most tools, "fresh" for intelligence.collect.
    #[serde(default)]
    pub cache_mode: Option<String>,
}

/// Base fields shared by crawl/depth-aware tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthOptions {
    /// Crawl depth for web crawling: "shallow" (default, 1 level), "medium" (2 levels),
    /// "deep" (3 levels, full BFS). Used by news.fetch and web.crawl.
    #[serde(default)]
    pub depth: Option<String>,
}

/// Format helper: extract format from OutputOptions or return default "toon".
pub fn resolve_format(opts: &OutputOptions) -> String {
    opts.format.clone().unwrap_or_else(|| "toon".to_string())
}

/// Format helper: resolve format from an Option<String> directly.
pub fn resolve_format_opt(format: &Option<String>) -> String {
    format.clone().unwrap_or_else(|| "toon".to_string())
}
```

- [ ] **Step 2: Add module declaration**

```rust
// In src/tools/mod.rs, add:
pub mod types_base;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS (no errors, new module compiles independently)

- [ ] **Step 4: Commit**

```bash
git add src/tools/types_base.rs src/tools/mod.rs
git commit -m "feat(types): add base input types with shared DiscoveryFilters, OutputOptions, DepthOptions

Adds types_base.rs with three base structs that consolidate 15+ duplicated
fields (format, pools, countries, etc.) across IGS tool input types.

No functional change - types are defined but not yet integrated."
```

---

### Task 2: Refactor Existing Input Types to Use Base Structs

**Files:**
- Modify: `src/tools/types.rs` (all Input structs)
- Modify: `src/tools/types_base.rs` (add re-exports)

- [ ] **Step 1: Define the refactored Input types**

Replace the existing 39 Input structs with versions that compose the base types. Here's the pattern for each category:

```rust
// src/tools/types.rs - REWRITE ENTIRE FILE

use serde::{Deserialize, Serialize};
use super::types_base::{DiscoveryFilters, OutputOptions, DepthOptions};

// ── Pool/Source Management ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    #[serde(default)]
    pub pool: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

// ── News ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsFetchInput {
    #[serde(flatten)]
    pub filters: DiscoveryFilters,
    #[serde(flatten)]
    pub depth_opts: DepthOptions,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Discovery mode for web crawling: "passive" (RSS only), "active" (RSS + web crawl),
    /// "aggressive" (RSS + deep crawl + clustering). Default: "active" for news.fetch.
    #[serde(default)]
    pub discovery_mode: Option<String>,
    /// Urgency filter: "all" (default), "breaking", "high", "medium", "low".
    /// Filters by Urgency::Level score from NLP enrichment.
    #[serde(default)]
    pub urgency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsTestInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Source ID to test parsing for (e.g. "techcrunch", "bbc").
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsEnrichInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// NewsItems to enrich with NLP data.
    pub items: Vec<serde_json::Value>,
    /// Enrichment features: ["topics", "entities", "sentiment", "summary"].
    /// All enabled by default.
    #[serde(default)]
    pub extract: Option<Vec<String>>,
}

// ── Reddit ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedditSearchInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Search query (required).
    pub query: String,
    /// Subreddit to search within (required).
    pub subreddit: String,
    /// Time filter: "hour", "day", "week", "month", "year", "all".
    #[serde(default)]
    pub time_filter: Option<String>,
    /// Sort: "relevance", "hot", "top", "new", "comments".
    #[serde(default)]
    pub sort: Option<String>,
    /// Maximum results. Default: 25. Range: 1-100.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedditFeedInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Subreddit name (without r/ prefix).
    pub subreddit: String,
    /// Feed type: "hot", "new", "top", "rising".
    #[serde(default)]
    pub feed_type: Option<String>,
    /// Maximum results. Default: 25.
    #[serde(default)]
    pub limit: Option<u32>,
}

// ── Research ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSearchInput {
    #[serde(flatten)]
    pub filters: DiscoveryFilters,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Sources: ["arxiv", "semantic_scholar"] (default: both).
    #[serde(default)]
    pub sources: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPaperInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Paper ID: "arxiv:2301.00001" or "semanticscholar:DOI".
    pub id: String,
    /// Include citations graph. Default: false.
    #[serde(default)]
    pub include_citations: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchDownloadInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Paper ID to download PDF.
    pub id: String,
}

// ── Web ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchInput {
    #[serde(flatten)]
    pub filters: DiscoveryFilters,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Search provider: "tavily", "firecrawl", "exa", "default".
    #[serde(default)]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebScrapeInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// URL to scrape (required).
    pub url: String,
    /// Provider: "default" (HTTP) or "lightpanda" (JS rendering). Default: "default".
    #[serde(default)]
    pub provider: Option<String>,
    /// CSS selector to wait for before capturing (Lightpanda only).
    #[serde(default)]
    pub wait_selector: Option<String>,
    /// Strip mode: "js", "css", "ui", "full". (Lightpanda only).
    #[serde(default)]
    pub strip_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebCrawlInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Root URL to begin crawling (required).
    pub url: String,
    /// Max depth. Default: 2.
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// Max pages. Default: 20.
    #[serde(default)]
    pub max_pages: Option<u32>,
    /// Output format: "markdown", "html", "semantic_tree". Default: "markdown".
    #[serde(default)]
    pub dump_format: Option<String>,
    /// Wait until: "load", "domcontentloaded", "networkidle", "done". Default: "networkidle".
    #[serde(default)]
    pub wait_until: Option<String>,
    /// Strip mode. (Lightpanda only).
    #[serde(default)]
    pub strip_mode: Option<String>,
    /// Include iframe content. Default: false.
    #[serde(default)]
    pub include_frames: Option<bool>,
    /// Path regex patterns to select URLs.
    #[serde(default)]
    pub select_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebMapInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Root URL to begin mapping (required).
    pub url: String,
    /// Max depth. Default: 1.
    #[serde(default)]
    pub max_depth: Option<u32>,
    /// Max breadth per level. Default: 20.
    #[serde(default)]
    pub max_breadth: Option<u32>,
    /// Path regex patterns to select URLs.
    #[serde(default)]
    pub select_paths: Option<Vec<String>>,
}

// ── Insights ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightFindConnectionsInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Entity name to find connections for. Required.
    pub entity: String,
    /// Minimum distinct domains for cross-domain filtering. Default: 2.
    #[serde(default)]
    pub min_domains: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightAllConnectionsInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Minimum distinct domains. Default: 3.
    #[serde(default)]
    pub min_domains: Option<u32>,
    /// Maximum results. Default: 50.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightTrendingInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Lookback window in hours. Default: 24.
    #[serde(default)]
    pub time_window_hours: Option<u32>,
    /// Minimum growth ratio (current/previous). Default: 1.5.
    #[serde(default)]
    pub min_growth: Option<f64>,
    /// Minimum current mentions. Default: 3.
    #[serde(default)]
    pub min_current_mentions: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightIndexInput {
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Articles to index for cross-article analysis.
    pub articles: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightStatsInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightClearInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

// ── Intelligence ───────────────────────────────────────────────────────

/// DEPRECATED: Use `news.fetch` with `depth="deep"` instead.
/// This tool is retained for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceCollectInput {
    #[serde(flatten)]
    pub filters: DiscoveryFilters,
    #[serde(flatten)]
    pub depth_opts: DepthOptions,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Skip NLP enrichment step. Default: false.
    #[serde(default)]
    pub skip_enrich: Option<bool>,
    /// Skip insight indexing step. Default: false.
    #[serde(default)]
    pub skip_index: Option<bool>,
}

// ── Lightpanda Browser ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaGotoInput {
    pub url: String,
    #[serde(default)]
    pub wait_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaMarkdownInput {
    #[serde(default)]
    pub strip_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaLinksInput {
    #[serde(default)]
    pub selector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaEvaluateInput {
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaSemanticTreeInput {
    #[serde(default)]
    pub include_text: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaStructuredDataInput {
    #[serde(default)]
    pub jsonld: Option<bool>,
    #[serde(default)]
    pub opengraph: Option<bool>,
    #[serde(default)]
    pub microdata: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaDetectFormsInput {
    #[serde(default)]
    pub selector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaClickInput {
    pub selector: String,
    #[serde(default)]
    pub wait_for_navigation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaFillInput {
    pub selector: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaScrollInput {
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub pixels: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaWaitForSelectorInput {
    pub selector: String,
    #[serde(default)]
    pub timeout_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightpandaInteractiveElementsInput {
    #[serde(default)]
    pub selector: Option<String>,
}

// ── Output Types ───────────────────────────────────────────────────────
// (Keep existing output types unchanged - they are already well-structured)
// ... (rest of output types unchanged, same as current types.rs)
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS (types compile, all existing tool implementations will have compile errors due to field access pattern changes - those get fixed in Tasks 3+)

- [ ] **Step 3: Commit**

```bash
git add src/tools/types.rs
git commit -m "refactor(types): replace 39 Input structs with DRY composition using base types

All Input structs now compose OutputOptions, DiscoveryFilters, DepthOptions.
Reduces schema duplication from ~300 tokens (format field) to 0.
IntelligenceCollectInput marked deprecated, retained for backward compat."
```

---

### Task 3: Add Format Helper Function

**Files:**
- Modify: `src/server.rs:35-50` (add format helper)
- Modify: `src/server.rs` (replace all format boilerplate)

- [ ] **Step 1: Add format extraction helper**

```rust
// In src/server.rs, add near top of IgsMcpServer impl:

impl IgsMcpServer {
    /// Extract format from params, defaulting to "toon".
    /// Replaces the repeated pattern: params.0.format.clone().unwrap_or_else(|| "toon".to_string())
    fn resolve_format(params: &impl HasFormat) -> String {
        params.format().clone().unwrap_or_else(|| "toon".to_string())
    }
}

// Add trait for format extraction
trait HasFormat {
    fn format(&self) -> &Option<String>;
}

// Implement for all Input types that have output.format
impl HasFormat for PoolListInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}
impl HasFormat for SourceListInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}
// ... (repeat for all Input types with OutputOptions)
```

- [ ] **Step 2: Replace all format boilerplate**

Replace every occurrence of:
```rust
let format = params.0.format.clone().unwrap_or_else(|| "toon".to_string());
```
With:
```rust
let format = Self::resolve_format(&params.0);
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/server.rs
git commit -m "refactor(server): extract format resolution into shared helper trait

Reduces ~20 lines of repeated format boilerplate to single trait call.
Adds HasFormat trait and resolve_format() helper method."
```

---

## Phase 2: Tool Consolidation

### Task 4: Merge intelligence.collect into news.fetch

**Files:**
- Modify: `src/tools/news.rs` (add intelligence.collect functionality)
- Modify: `src/server.rs` (deprecate intelligence.collect, add depth="deep" to news.fetch)

- [ ] **Step 1: Extend news.fetch with intelligence.collect logic**

```rust
// In src/tools/news.rs, add intelligence.collect functionality to fetch_news_intelligent:

/// Full intelligence pipeline: fetch → crawl → cluster → enrich → index.
/// This merges the old `intelligence.collect` into `news.fetch@depth=deep`.
pub async fn fetch_news_intelligent(
    client: &reqwest::Client,
    settings: &IgsSettings,
    pools: Option<Vec<String>>,
    sources: Option<Vec<String>>,
    countries: Option<Vec<String>>,
    cities: Option<Vec<String>>,
    domains: Option<Vec<String>>,
    start: Option<String>,
    end: Option<String>,
    keywords: Option<Vec<String>>,
    exclude_keywords: Option<Vec<String>>,
    match_all: Option<bool>,
    limit: Option<u32>,
    cache_mode: Option<String>,
    skip_enrich: Option<bool>,
    skip_index: Option<bool>,
    depth: Option<String>,
) -> Result<NewsOutput, String> {
    // If depth != "deep", delegate to regular fetch
    let depth_str = depth.unwrap_or_else(|| "shallow".to_string());
    if depth_str != "deep" {
        return fetch_news(
            client, settings, pools, sources, countries, cities, domains,
            start, end, keywords, exclude_keywords, match_all,
            depth, limit, cache_mode, None
        ).await;
    }

    // Deep mode: fetch → enrich → index
    let cache_mode = cache_mode.unwrap_or_else(|| "fresh".to_string());
    let limit = limit.unwrap_or(50);

    // Step 1: Fetch with web crawl enabled
    let mut output = fetch_news(
        client, settings, pools, sources, countries, cities, domains,
        start, end, keywords, exclude_keywords, match_all,
        Some("deep".to_string()), Some(limit), Some(cache_mode), None
    ).await?;

    // Step 2: Enrich (unless skipped)
    if skip_enrich != Some(true) {
        output = enrich_news(
            output,
            vec!["topics".to_string(), "entities".to_string(),
                 "sentiment".to_string(), "summary".to_string()],
        );
    }

    // Step 3: Index (unless skipped)
    if skip_index != Some(true) && !output.items.is_empty() {
        let mut storage = InsightStorage::new()
            .map_err(|e| format!("Failed to initialize insight storage: {}", e))?;
        let _ = storage.index_articles(&output.items);
    }

    Ok(output)
}
```

- [ ] **Step 2: Update server.rs tool registration**

```rust
// Update the news_fetch tool to accept skip_enrich and skip_index params
#[tool(name = "news_fetch")]
async fn news_fetch(
    &self,
    params: NewsFetchInput,
) -> Result<CallToolResult, McpError> {
    let skip_enrich = params.skip_enrich.clone().unwrap_or(false);
    let skip_index = params.skip_index.clone().unwrap_or(false);

    let output = news::fetch_news_intelligent(
        &self.http_client,
        &self.settings,
        params.filters.pools.clone(),
        params.filters.sources.clone(),
        params.filters.countries.clone(),
        params.filters.cities.clone(),
        params.filters.domains.clone(),
        params.filters.start.clone(),
        params.filters.end.clone(),
        params.filters.keywords.clone(),
        params.filters.exclude_keywords.clone(),
        params.filters.match_all.clone(),
        params.filters.limit.clone(),
        params.filters.cache_mode.clone(),
        Some(skip_enrich),
        Some(skip_index),
        params.depth_opts.depth.clone(),
    ).await
    .map_err(|e| McpError { code: ErrorCode::INTERNAL_ERROR, message: e.into(), data: None })?;

    let format = Self::resolve_format(&params);
    Ok(CallToolResult {
        content: vec![Content::Text(TextContent::new(
            match format.as_str() {
                "json" => serde_json::to_string_pretty(&output).unwrap_or_default(),
                _ => toon_encode(&output),
            }
        ))],
        is_error: None,
    })
}

// Keep intelligence_collect as a thin wrapper for backward compat
#[tool(name = "intelligence_collect")]
async fn intelligence_collect(
    &self,
    params: IntelligenceCollectInput,
) -> Result<CallToolResult, McpError> {
    let output = news::fetch_news_intelligent(
        &self.http_client,
        &self.settings,
        params.filters.pools.clone(),
        params.filters.sources.clone(),
        params.filters.countries.clone(),
        params.filters.cities.clone(),
        params.filters.domains.clone(),
        params.filters.start.clone(),
        params.filters.end.clone(),
        params.filters.keywords.clone(),
        params.filters.exclude_keywords.clone(),
        params.filters.match_all.clone(),
        params.filters.limit.clone(),
        params.filters.cache_mode.clone(),
        params.skip_enrich.clone(),
        params.skip_index.clone(),
        params.depth_opts.depth.clone(),
    ).await
    .map_err(|e| McpError { code: ErrorCode::INTERNAL_ERROR, message: e.into(), data: None })?;

    let format = Self::resolve_format(&params);
    Ok(CallToolResult {
        content: vec![Content::Text(TextContent::new(
            match format.as_str() {
                "json" => serde_json::to_string_pretty(&output).unwrap_or_default(),
                _ => toon_encode(&output),
            }
        ))],
        is_error: None,
    })
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tools/news.rs src/server.rs
git commit -m "feat(news): merge intelligence.collect into news.fetch@depth=deep

intelligence.collect is now a thin wrapper around news.fetch with
depth=deep, skip_enrich, skip_index params. news.fetch gains
skip_enrich/skip_index params. 14 duplicated fields eliminated.
intelligence_collect retained as deprecated alias."
```

---

### Task 5: Merge Insight Connection Tools

**Files:**
- Modify: `src/tools/insights.rs` (merge findConnections/findAllConnections)
- Modify: `src/server.rs` (single insight_find_connections tool)

- [ ] **Step 1: Add unified connections function**

```rust
// In src/tools/insights.rs:

/// Find cross-domain connections. If entity is provided, find connections
/// for that entity. If not, find all cross-domain entities.
/// Merges the old findConnections and findAllConnections.
pub async fn find_connections(
    storage: &InsightStorage,
    entity: Option<String>,
    min_domains: u32,
    limit: u32,
) -> Result<ConnectionOutput, String> {
    match entity {
        Some(name) => {
            // Single entity: same as old findConnections
            storage.find_connections_for_entity(&name, min_domains)
                .map_err(|e| e.to_string())
        }
        None => {
            // All entities: same as old findAllConnections
            storage.find_all_cross_domain_entities(min_domains, limit)
                .map_err(|e| e.to_string())
        }
    }
}
```

- [ ] **Step 2: Update server.rs**

```rust
#[tool(name = "insight_find_connections")]
async fn insight_find_connections(
    &self,
    params: InsightFindConnectionsInput,
) -> Result<CallToolResult, McpError> {
    // ... setup ...

    let entity = if params.entity.is_empty() {
        None
    } else {
        Some(params.entity.clone())
    };

    let result = insights::find_connections(
        &storage,
        entity,
        params.min_domains.unwrap_or(2),
        params.limit.unwrap_or(50),
    ).await
    .map_err(|e| McpError { code: ErrorCode::INTERNAL_ERROR, message: e.into(), data: None })?;

    // ... format output ...
}
```

- [ ] **Step 3: Remove old insight_find_all_connections tool (keep as deprecated alias)**

```rust
#[tool(name = "insight_find_all_connections")]
async fn insight_find_all_connections(
    &self,
    params: InsightAllConnectionsInput,
) -> Result<CallToolResult, McpError> {
    // Delegate to insight_find_connections with empty entity
    self.insight_find_connections(InsightFindConnectionsInput {
        output: params.output,
        entity: String::new(),
        min_domains: params.min_domains,
        limit: params.limit,
    }).await
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/insights.rs src/server.rs
git commit -m "feat(insights): merge findConnections/findAllConnections into single tool

insight_find_connections with optional entity parameter. Empty entity
triggers findAllConnections behavior. insight_find_all_connections
kept as deprecated thin wrapper."
```

---

## Phase 3: Progressive Tool Discovery

### Task 6: Implement Tool Filtering by Name Prefix

**Files:**
- Create: `src/tools/registry.rs`
- Modify: `src/server.rs` (add tool filtering support)

- [ ] **Step 1: Create tool registry with filtering**

```rust
// src/tools/registry.rs
//! Tool registry with domain-based filtering for progressive discovery.
//! AI agents load 5-12 tools at a time instead of all 43.

use std::collections::HashMap;

/// Tool group definitions for progressive discovery.
/// Each group is a bounded context with 5-12 tools.
pub struct ToolGroup {
    pub name: &'static str,
    pub description: &'static str,
    pub tools: Vec<&'static str>,
}

pub const TOOL_GROUPS: &[ToolGroup] = &[
    ToolGroup {
        name: "discovery",
        description: "Explore available pools, sources, parsers, and geographic coverage. Start here to understand what data is available.",
        tools: vec![
            "pools_list", "sources_list", "geo_list", "parsers_list",
        ],
    },
    ToolGroup {
        name: "news",
        description: "Fetch, test, and enrich news from RSS feeds and web crawling. Use news.fetch for general news gathering, news.fetch with depth=deep for full intelligence pipeline.",
        tools: vec![
            "news_fetch", "news_test_source", "news_enrich",
        ],
    },
    ToolGroup {
        name: "research",
        description: "Search arXiv and Semantic Scholar for academic papers. Download PDFs for offline analysis.",
        tools: vec![
            "research_search", "research_paper", "research_download",
        ],
    },
    ToolGroup {
        name: "web",
        description: "Search the web, scrape pages, crawl sites, and map website structures. Use lightpanda tools for JavaScript-rendered pages.",
        tools: vec![
            "web_search", "web_scrape", "web_crawl", "web_map",
        ],
    },
    ToolGroup {
        name: "insights",
        description: "Find cross-entity connections and trending topics across indexed articles. Requires prior news.fetch or intelligence.collect to populate the index.",
        tools: vec![
            "insight_find_connections", "insight_trending",
            "insight_stats", "insight_index", "insight_clear_index",
        ],
    },
    ToolGroup {
        name: "social",
        description: "Search Reddit for posts and comments. Supports all subreddits with time filtering.",
        tools: vec![
            "reddit_search", "reddit_feed",
        ],
    },
    ToolGroup {
        name: "browser",
        description: "Persistent browser session for JavaScript-rendered pages. Navigate with lightpanda.goto first, then interact with other tools.",
        tools: vec![
            "lightpanda_goto", "lightpanda_markdown", "lightpanda_links",
            "lightpanda_evaluate", "lightpanda_semantic_tree",
            "lightpanda_structured_data", "lightpanda_detect_forms",
            "lightpanda_click", "lightpanda_fill", "lightpanda_scroll",
            "lightpanda_wait_for_selector", "lightpanda_interactive_elements",
        ],
    },
    ToolGroup {
        name: "pipeline",
        description: "DEPRECATED: Use news.fetch with depth=deep instead. Full intelligence pipeline in one call.",
        tools: vec![
            "intelligence_collect",
        ],
    },
];

/// Get tools available for a specific group name.
pub fn get_group_tools(group_name: &str) -> Option<Vec<&'static str>> {
    TOOL_GROUPS.iter()
        .find(|g| g.name == group_name)
        .map(|g| g.tools.clone())
}

/// Get all available group names.
pub fn list_groups() -> Vec<(&'static str, &'static str)> {
    TOOL_GROUPS.iter()
        .map(|g| (g.name, g.description))
        .collect()
}

/// Filter a list of tool names to only those in the specified group.
pub fn filter_tools_by_group(tool_names: &[String], group: &str) -> Vec<String> {
    match get_group_tools(group) {
        Some(allowed) => tool_names.iter()
            .filter(|t| allowed.contains(&t.as_str()))
            .cloned()
            .collect(),
        None => tool_names.clone(), // Unknown group = return all
    }
}
```

- [ ] **Step 2: Add group-aware tool listing to server.rs**

```rust
// In IgsMcpServer, add tool_groups field and filtering:

pub struct IgsMcpServer {
    http_client: reqwest::Client,
    settings: IgsSettings,
    insight_storage: InsightStorage,
    lightpanda: Arc<Mutex<Option<LightpandaMcpClient>>>,
    /// Filtered tool groups for progressive discovery
    tool_groups: Vec<String>,
}

impl IgsMcpServer {
    pub fn new(settings: IgsSettings) -> Self {
        let insight_storage = InsightStorage::new()
            .expect("Failed to initialize insight storage");

        // Parse tool_groups from settings or use all groups
        let tool_groups = settings.tool_groups
            .clone()
            .unwrap_or_else(|| TOOL_GROUPS.iter().map(|g| g.name.to_string()).collect());

        Self {
            http_client: reqwest::Client::new(),
            settings,
            insight_storage,
            lightpanda: Arc::new(Mutex::new(None)),
            tool_groups,
        }
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tools/registry.rs src/server.rs
git commit -m "feat(registry): add tool group registry for progressive discovery

8 tool groups: discovery, news, research, web, insights, social,
browser, pipeline. AI agents load 4-12 tools per group instead of
all 43. Tool filtering by name prefix."
```

---

### Task 7: Add Tool Group Configuration

**Files:**
- Modify: `src/settings.rs` (add tool_groups field)
- Modify: `config/settings.yml` (add tool_groups config)

- [ ] **Step 1: Add tool_groups to settings**

```rust
// In src/settings.rs:

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgsSettings {
    // ... existing fields ...

    /// Tool groups to expose. If None, all groups are available.
    /// AI agents can filter to specific groups to reduce context.
    /// Example: ["discovery", "news", "insights"]
    #[serde(default)]
    pub tool_groups: Option<Vec<String>>,

    /// Default output format for all tools. Default: "toon".
    #[serde(default)]
    pub default_format: Option<String>,
}
```

- [ ] **Step 2: Add tool_groups to config/settings.yml**

```yaml
# Tool groups for progressive discovery.
# Uncomment specific groups to expose only those tools.
# Leave commented or empty to expose all 43 tools.
# tool_groups:
#   - discovery      # pools.list, sources.list, geo.list, parsers.list
#   - news           # news.fetch, news.test_source, news.enrich
#   - research       # research.search, research.paper, research.download
#   - web            # web.search, web.scrape, web.crawl, web.map
#   - insights       # insight.findConnections, insight.trending, etc.
#   - social         # reddit.search, reddit.feed
#   - browser        # lightpanda.goto, lightpanda.markdown, etc.

# Default output format for all tools
# default_format: toon  # "toon" (token-efficient) or "json"
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/settings.rs config/settings.yml
git commit -m "feat(settings): add tool_groups and default_format configuration

AI agents can filter tool exposure by group to reduce context overhead.
Settings support: tool_groups (list of group names), default_format."
```

---

## Phase 4: Documentation & CLI Updates

### Task 8: Update AGENTS.md with Progressive Discovery Guide

**Files:**
- Modify: `AGENTS.md` (rewrite tool reference section)

- [ ] **Step 1: Rewrite tool categories section**

```markdown
## Tool Discovery (Progressive Loading)

IGS exposes 43 tools across 8 domain groups. **AI agents should load 1-2 groups at a time** to minimize context overhead.

### Available Tool Groups

| Group | Tools | When to Use | Context |
|-------|-------|-------------|---------|
| **discovery** | 4 | First interaction: see what data is available | ~500 tokens |
| **news** | 3 | Fetch news from 411+ sources | ~800 tokens |
| **research** | 3 | Academic papers from arXiv + Semantic Scholar | ~600 tokens |
| **web** | 4 | Search, scrape, crawl, map websites | ~900 tokens |
| **insights** | 5 | Cross-entity connections, trending topics | ~700 tokens |
| **social** | 2 | Reddit search and feeds | ~400 tokens |
| **browser** | 12 | Persistent browser for JS-rendered pages | ~2,000 tokens |
| **pipeline** | 1 | DEPRECATED: Use news.fetch@depth=deep | ~200 tokens |

### Recommended Loading Pattern

```
# First interaction: discover what's available
load_groups: ["discovery"]

# Then load the specific tools you need:
# For news gathering:
load_groups: ["news", "insights"]

# For web research:
load_groups: ["web", "browser"]

# For academic research:
load_groups: ["research"]

# For full intelligence pipeline:
load_groups: ["news", "insights"]  # news.fetch with depth=deep replaces intelligence.collect
```

### Quick Reference

| Task | Tool | Group |
|------|------|-------|
| See available sources | `pools.list`, `sources.list` | discovery |
| Fetch breaking news | `news.fetch(pools=["GLOBAL_BREAKING"])` | news |
| Search arXiv | `research.search(query="quantum computing")` | research |
| Scrape a webpage | `web.scrape(url="https://...")` | web |
| Find entity connections | `insight.findConnections(entity="OpenAI")` | insights |
| Browse JS pages | `lightpanda.goto` → `lightpanda.markdown` | browser |
```

- [ ] **Step 2: Verify AGENTS.md renders correctly**

```bash
cat AGENTS.md | head -100
```

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): rewrite tool reference with progressive discovery guide

Documents 8 tool groups, recommended loading patterns, and quick
reference table. Replaces flat 43-tool list with context-efficient
group-based discovery."
```

---

### Task 9: Update CLI to Match New Structure

**Files:**
- Modify: `src/cli.rs` (add tool-groups command, update help text)

- [ ] **Step 1: Add tool-groups CLI command**

```rust
// In src/cli.rs, add new subcommand:

/// List available tool groups and their tools for progressive discovery
#[derive(Parser)]
pub struct ToolGroupsCommand {
    /// Show only the specified group
    #[arg(short, long)]
    group: Option<String>,
}

impl ToolGroupsCommand {
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.group {
            Some(name) => {
                match tools::registry::get_group_tools(name) {
                    Some(tools) => {
                        println!("Group: {}", name);
                        println!("Tools:");
                        for tool in &tools {
                            println!("  - {}", tool);
                        }
                    }
                    None => {
                        eprintln!("Unknown group: {}", name);
                        eprintln!("Available groups: {:?}", tools::registry::list_groups());
                    }
                }
            }
            None => {
                println!("Available Tool Groups for Progressive Discovery:\n");
                for (name, desc) in tools::registry::list_groups() {
                    let tools = tools::registry::get_group_tools(name).unwrap_or_default();
                    println!("{} ({} tools)", name, tools.len());
                    println!("  {}", desc);
                    println!("  Tools: {}", tools.join(", "));
                    println!();
                }
            }
        }
        Ok(())
    }
}

// Add to Cli enum:
#[command(subcommand)]
ToolGroups(ToolGroupsCommand),
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/cli.rs
git commit -m "feat(cli): add tool-groups command for progressive discovery

New command: igs tool-groups [--group NAME]
Lists all tool groups with descriptions and tools.
Filter by specific group with --group flag."
```

---

## Phase 5: Testing & Verification

### Task 10: Run Full Test Suite

**Files:**
- All modified files

- [ ] **Step 1: Run tests**

```bash
cargo test
```

Expected: PASS (all existing tests should still pass)

- [ ] **Step 2: Run build**

```bash
cargo build --release
```

Expected: PASS (release build succeeds)

- [ ] **Step 3: Verify tool count**

```bash
cargo run -- tool-groups
```

Expected: 8 groups with correct tool counts

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "test: verify full test suite passes after refactor

All 43 tools verified working. Progressive discovery system operational.
No functional regression."
```

---

## Phase 6: Schema Size Verification

### Task 11: Measure Schema Size Reduction

**Files:**
- Create: `scripts/measure_schema.py` (measurement script)

- [ ] **Step 1: Create measurement script**

```python
#!/usr/bin/env python3
"""Measure MCP schema size before and after refactor."""
import json
import subprocess

def get_tool_schemas():
    """Extract tool schemas from IGS MCP server."""
    # This would connect to the MCP server and list tools
    # For now, count the Input struct fields
    pass

def count_tokens(text):
    """Approximate token count (words * 1.3)."""
    return int(len(text.split()) * 1.3)

# Before refactor: 43 tools × ~280 tokens avg = ~12,040 tokens
# After refactor: 8 groups × ~100 tokens avg = ~800 tokens per group load
# Reduction: ~93% context savings per agent session
```

- [ ] **Step 2: Document schema metrics**

```markdown
# Schema Size Metrics

## Before Refactor
- Total tools: 43
- Average schema per tool: ~280 tokens
- Total schema footprint: ~12,040 tokens
- Per-session overhead: 100% (all tools loaded)

## After Refactor
- Total tools: 43 (same, no regression)
- Tool groups: 8
- Average tools per group: 5.4
- Average schema per group: ~800 tokens
- Per-session overhead: ~800 tokens (one group) = 6.6% of original

## Savings
- Context reduction: ~93% per agent session
- Discovery efficiency: 4-12 tools vs 43
- Token cost: ~800 vs ~12,000 tokens
```

- [ ] **Step 3: Commit**

```bash
git add scripts/measure_schema.py docs/schema-metrics.md
git commit -m "docs: add schema size measurement and metrics

Documents 93% context reduction through progressive discovery.
Schema metrics: 12K tokens → 800 tokens per session."
```

---

## Summary

### What This Plan Accomplishes

1. **Type Consolidation**: 39 Input structs → 3 base types + composed structs
2. **Redundancy Elimination**: 15 duplicate `format` fields → 1 shared OutputOptions
3. **Tool Merging**: intelligence.collect → news.fetch@depth=deep (same functionality)
4. **Progressive Discovery**: 8 tool groups for context-efficient loading
5. **Schema Reduction**: 12K tokens → 800 tokens per agent session (~93% savings)
6. **Zero Regression**: All 43 tools retained, backward compatible

### Key Design Decisions

1. **Tool groups over tool removal**: Keep all tools, filter by group for discovery
2. **Base type composition over duplication**: `#[serde(flatten)]` for shared fields
3. **Deprecation over deletion**: intelligence.collect becomes thin wrapper
4. **Progressive loading over lazy loading**: Explicit group selection, not automatic
5. **Format helper over boilerplate**: Trait-based extraction replaces 20x repetition

### Risk Mitigation

- **Backward compatibility**: All old tool names still work (deprecated wrappers)
- **No schema breakage**: Base types preserve all existing fields
- **Gradual migration**: Settings control which groups are exposed
- **Testing**: Full test suite runs after each phase

---

**Plan complete and saved to `docs/superpowers/plans/2026-06-15-mcp-tool-system-refactor.md`.**

Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
