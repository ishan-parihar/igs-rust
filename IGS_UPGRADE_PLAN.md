# IGS MCP Server — Strategic Upgrade Plan

**Version**: 1.0  
**Date**: 2026-06-19  
**Status**: Draft  
**Reference**: ANX Protocol (arXiv:2604.04820) — Protocol-First Design for AI Agent Interaction

---

## Executive Summary

This document outlines a comprehensive upgrade plan for the IGS (Intelligence Gathering System) MCP server. Based on analysis of the current 41-tool architecture, the ANX protocol paper's findings on token efficiency and agent-native design, and MCP best practices from production servers, we propose a three-phase upgrade to transform IGS from a functional tool collection into a highly efficient, scalable intelligence platform.

**Key findings**:
- Current architecture has 28% context overhead when all tools are loaded
- HttpClient is re-created 16x per session (TLS/connection pool waste)
- Settings are re-loaded 23+ times per session (disk I/O waste)
- Format output boilerplate duplicated 28 times (~200 lines of dead weight)
- No `tool_guide` meta-tool for progressive discovery (proven pattern for 40+ tool servers)

**Target state**: A protocol-first, agent-native intelligence platform with <10% baseline context cost, zero redundant I/O, and dynamic tool discovery.

---

## 1. Current State Assessment

### 1.1 Architecture Overview

| Metric | Current Value |
|--------|---------------|
| Total Tools | 41 (29 core + 12 Lightpanda) |
| Tool Groups | 7 (discovery, news, research, web, insights, social, browser) |
| Context Overhead | ~28% when all groups loaded |
| Codebase Size | 9,333 lines Rust across 28 files |
| Dependencies | 22 crates |
| Binary | Single `igs` binary (musl static) |

### 1.2 Identified Anti-Patterns

#### P0 — Critical Performance Issues

| Issue | Impact | Location |
|-------|--------|----------|
| HttpClient re-created per tool call (16x) | Fresh TLS context + connection pool each time | `web.rs`, `news.rs`, `research.rs` |
| Settings loaded from disk 23+ times/session | Disk I/O per tool call | `server.rs` all handlers |
| Lightpanda `ensure_ready()` called 12x | Redundant binary check + settings load | `server.rs:743-824` |

#### P1 — Code Duplication

| Issue | Impact | Location |
|-------|--------|----------|
| Format output boilerplate (28 copies) | ~200 lines dead weight | `server.rs:348-824` |
| `dump::maybe_dump` boilerplate (13 copies) | Redundant settings loads | `server.rs` all handlers |
| `CallToolResult` wrapping (28 copies) | Identical pattern repeated | `server.rs:348-824` |

#### P2 — Scaling Concerns

| Issue | Impact | Location |
|-------|--------|----------|
| InsightStorage Vec+Mutex for all reads | O(n) scans, no concurrency | `server.rs:20-231` |
| `batch_similar()` O(n²) | Degrades at depth=deep scale | `parsers.rs:1119-1175` |
| No batch SQLite writes | N individual INSERT statements | `insights.rs:57-67` |
| No LRU eviction for file cache | Unbounded disk usage | `cache.rs` |

#### P3 — Architectural Gaps

| Issue | Impact | Location |
|-------|--------|----------|
| Tool registry not validated against `#[tool]` | Silent mismatches possible | `registry.rs` vs `server.rs` |
| String error types everywhere | No structured error handling | All tool functions |
| No `tool_guide` meta-tool | LLM must guess which tool to use | Missing |
| No cursor-based pagination | All 41 tools returned at once | `registry.rs` |

### 1.3 What Works Well

- **Domain-based tool grouping** — Clean 7-group taxonomy in `registry.rs`
- **Progressive loading via `tool_groups`** — Settings-driven tool filtering
- **Dual-tier caching** — FeedCache + QueryCache with ETag support
- **TOON output format** — ~40% fewer tokens than JSON
- **Per-host concurrency limiting** — Prevents downstream abuse
- **Bootstrap pattern** — First-run works without manual setup
- **Single binary deployment** — musl static, zero runtime deps

---

## 2. Reference Architecture: ANX Protocol Insights

The ANX paper (arXiv:2604.04820) provides critical insights for agent-native protocol design:

### 2.1 Key Findings from ANX

| Finding | Implication for IGS |
|---------|---------------------|
| ANX reduces tokens by 47-57% vs MCP | TOON format already achieves ~40%; room for improvement via tool_guide |
| 3EX decoupled architecture (Expression-Exchange-Execution) | Separate tool discovery from tool execution |
| Dynamic discovery without pre-registration | Implement `tool_guide` meta-tool for on-demand tool selection |
| UI-to-Core sensitive data isolation | API keys should never enter LLM context |
| Machine-executable SOPs | Structured tool chains for multi-step intelligence workflows |
| Human-only confirmation gates | Destructive operations (delete source, clear index) need confirmation |

### 2.2 Applicable ANX Principles

1. **Protocol-First Design** — Define tool interfaces before implementation
2. **Progressive Disclosure** — Start with minimal tool set, expand on demand
3. **Semantic Precision** — Tool descriptions must be unambiguous for LLM routing
4. **Security by Default** — API keys isolated from agent context
5. **Marketplace-Driven Extensibility** — Tools as composable, discoverable units

---

## 3. MCP Best Practices (Production-Validated)

### 3.1 The Tool Guide Pattern

**The single most effective technique for 40+ tool servers.** Proven in production at OpenVisualCloud's 70+ diagnostic tool server.

```
┌─────────────────────────────────────────────────────────────┐
│  LLM calls tool_guide()                                     │
│  → Returns: decision_tree + categories + drill_down_chains  │
│  → Context cost: ~3 tool definitions                        │
│                                                             │
│  LLM calls specific_tool() based on guide                   │
│  → Only loads the tools it actually needs                   │
│  → Context cost: +1 tool definition per call                │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Official MCP Specification Requirements

| Requirement | IGS Current Status | Action Needed |
|-------------|-------------------|---------------|
| Tool names: 1-128 chars, unique | ✅ Compliant | None |
| `tools` capability declaration | ✅ Via rmcp crate | None |
| `listChanged: true` for dynamic tools | ❌ Not declared | Add capability |
| Cursor-based pagination for >20 tools | ❌ Not implemented | Add pagination |
| `isError: true` for tool execution errors | ❌ Returns `Result<T, String>` | Refactor error handling |
| Progress tokens for long operations | ❌ Not implemented | Add for depth=deep |
| `outputSchema` for typed results | ❌ Not used | Add to all tools |
| `annotations` for tool metadata | ❌ Not used | Add readOnlyHint, etc. |

### 3.3 Error Handling: Two-Tier Model

```
Protocol Errors (JSON-RPC level):
  → Unknown tool, invalid params, structural issues
  → Returns error code + message

Tool Execution Errors (in result):
  → Business logic failures, self-correctable
  → Returns isError: true + actionable feedback
  → LLM can retry with adjusted parameters
```

---

## 4. Upgrade Plan

### Phase 1: Foundation (Weeks 1-2)

**Goal**: Eliminate P0 performance issues, establish shared state architecture.

#### 1.1 Extract Shared State into `IgsMcpServer`

```rust
pub struct IgsMcpServer {
    // NEW: Shared state (loaded once at startup)
    pub settings: Settings,
    pub http_client: HttpClient,
    pub lightpanda: LightpandaManager,
    
    // EXISTING
    pub insight_storage: InsightStorage,
    pub tool_groups: Vec<String>,
}
```

**Impact**: Eliminates 16 HttpClient creations, 23+ settings loads, 12 Lightpanda ensure_ready calls.

#### 1.2 Add `format_output<T: Serialize>()` Helper

```rust
fn format_output<T: Serialize>(value: &T, format: &str, subject: &str) -> CallToolResult {
    let text = if format == "json" {
        serde_json::to_string_pretty(value).unwrap_or_default()
    } else {
        toon_encode(value)
    };
    
    #[cfg(not(test))]
    if let Ok(settings) = crate::config::load_settings().await {
        crate::tools::dump::maybe_dump(&settings, "tool.name", subject, &text);
    }
    
    CallToolResult::success(vec![Content::text(text)])
}
```

**Impact**: Eliminates ~200 lines of boilerplate from `server.rs`.

#### 1.3 Implement Cursor-Based Pagination

```rust
#[tool(name = "sources.list", description = "...")]
async fn sources_list(&self, params: Parameters<SourcesListInput>) -> Result<CallToolResult, String> {
    let all_sources = load_sources().await?;
    let page_size = params.page_size.unwrap_or(50);
    let (page, next_cursor) = paginate(&all_sources, params.cursor, page_size);
    
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&SourcesListOutput {
            sources: page,
            next_cursor,
            total: all_sources.len(),
        }).unwrap_or_default()
    )]))
}
```

#### 1.4 Add Tool Registry Validation Test

```rust
#[test]
fn test_registry_matches_registered_tools() {
    let registered_tools = get_all_tool_names(); // From #[tool] annotations
    let registry_tools = TOOL_GROUPS.iter()
        .flat_map(|g| g.tools.iter())
        .cloned()
        .collect::<HashSet<_>>();
    
    assert_eq!(registered_tools, registry_tools, 
        "Registry tools must match registered #[tool] annotations");
}
```

**Deliverables**:
- [ ] `IgsMcpServer` with shared state
- [ ] `format_output()` helper function
- [ ] Cursor-based pagination for all list tools
- [ ] Registry validation test
- [ ] Benchmarks: measure before/after for P0 fixes

---

### Phase 2: Intelligence (Weeks 3-4)

**Goal**: Add `tool_guide` meta-tool, implement new intelligence domains, improve error handling.

#### 2.1 Implement `tool_guide` Meta-Tool

```rust
#[tool(name = "tool.guide", description = "Categorized tool index with decision tree. Call this first to find the right tool for your task.")]
async fn tool_guide(&self, params: Parameters<ToolGuideInput>) -> Result<CallToolResult, String> {
    let guide = ToolGuide {
        decision_tree: HashMap::from([
            ("Fetch news articles", "news.fetch"),
            ("Search academic papers", "research.search"),
            ("Scrape a website", "web.scrape"),
            ("Find cross-domain connections", "insights.findConnections"),
            ("Monitor Reddit", "reddit.search"),
            ("Browse JS-rendered pages", "lightpanda.goto"),
        ]),
        categories: self.get_tool_categories(),
        drill_down_chains: vec![
            DrillDownChain {
                name: "Deep Research Pipeline",
                steps: vec!["web.search", "web.scrape", "news.enrich", "insights.indexArticles"],
            },
        ],
    };
    
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&guide).unwrap_or_default()
    )]))
}
```

#### 2.2 Add Weather Intelligence Domain

```rust
// src/tools/weather.rs
pub struct WeatherForecastInput {
    pub location: String,
    pub days: Option<u32>,
    pub format: Option<String>,
}

pub struct WeatherForecastOutput {
    pub location: String,
    pub forecasts: Vec<WeatherDay>,
    pub alerts: Vec<WeatherAlert>,
}

// API: OpenWeatherMap (free tier: 1000 calls/day)
// Parser: Direct JSON response parsing
// Pool: GLOBAL_ENVIRONMENT (complements existing climate sources)
```

**New tools**: `weather.forecast`, `weather.alerts`, `weather.current`

#### 2.3 Add Financial Intelligence Domain

```rust
// src/tools/finance.rs
pub struct MarketQuoteInput {
    pub symbols: Vec<String>,
    pub format: Option<String>,
}

pub struct MarketQuoteOutput {
    pub quotes: Vec<MarketQuote>,
    pub timestamp: String,
}

// APIs: Yahoo Finance (free), CoinGecko (free tier)
// New tools: finance.market, finance.crypto, finance.trending
```

#### 2.4 Add CVE/Security Intelligence Domain

```rust
// src/tools/security.rs
pub struct CveSearchInput {
    pub query: String,
    pub severity: Option<String>,
    pub days_back: Option<u32>,
}

pub struct CveSearchOutput {
    pub vulnerabilities: Vec<CveEntry>,
    pub total: usize,
}

// API: NVD (free, no key required for basic)
// New tools: security.cve, security.advisories
```

#### 2.5 Refactor Error Handling

```rust
// src/error.rs — New structured error type
pub enum IgsError {
    /// Tool execution error — LLM can retry
    ToolExecution {
        message: String,
        suggestion: String,
        retryable: bool,
    },
    /// Configuration error — needs human intervention
    Configuration {
        message: String,
        missing_key: Option<String>,
    },
    /// External API error
    ExternalApi {
        service: String,
        status: u16,
        message: String,
    },
}

impl Into<CallToolResult> for IgsError {
    fn into(self) -> CallToolResult {
        match self {
            IgsError::ToolExecution { message, suggestion, .. } => {
                CallToolResult {
                    content: vec![Content::text(format!(
                        "Error: {}\n\nSuggestion: {}\n\nRetry with adjusted parameters.",
                        message, suggestion
                    ))],
                    is_error: Some(true),
                    ..Default::default()
                }
            }
            // ... other variants
        }
    }
}
```

**Deliverables**:
- [ ] `tool_guide` meta-tool with decision tree
- [ ] Weather domain (3 tools + OpenWeatherMap integration)
- [ ] Financial domain (3 tools + Yahoo Finance/CoinGecko)
- [ ] Security domain (2 tools + NVD integration)
- [ ] Structured `IgsError` type with actionable feedback
- [ ] `isError: true` for all tool execution errors

---

### Phase 3: Scale (Weeks 5-6)

**Goal**: Optimize for scale, add advanced capabilities, prepare for production.

#### 3.1 InsightStorage Optimization

```rust
// Replace Vec+Mutex with indexed storage
pub struct InsightStorage {
    articles: Vec<Article>,           // Keep for fast iteration
    entity_index: HashMap<String, Vec<usize>>,  // NEW: Entity → article indices
    domain_index: HashMap<String, Vec<usize>>,  // NEW: Domain → article indices
    time_index: BTreeMap<i64, Vec<usize>>,       // NEW: Timestamp → article indices
}

impl InsightStorage {
    pub fn find_connections(&self, entity: Option<&str>, min_domains: usize) -> Vec<Connection> {
        if let Some(entity) = entity {
            // O(1) lookup via entity_index instead of O(n) scan
            let indices = self.entity_index.get(entity).unwrap_or(&vec![]);
            // ... process only relevant articles
        }
    }
}
```

#### 3.2 Batch SQLite Writes

```rust
pub async fn insights_index_batch(&self, articles: &[Article]) -> Result<(), IgsError> {
    let mut tx = self.db.begin().await?;
    
    for article in articles {
        sqlx::query("INSERT OR REPLACE INTO articles (id, title, content, ...) VALUES (?, ?, ?, ...)")
            .bind(&article.id)
            .bind(&article.title)
            .execute(&mut *tx)
            .await?;
    }
    
    tx.commit().await?;
    Ok(())
}
```

#### 3.3 LRU Cache Eviction

```rust
pub struct FeedCache {
    dir: PathBuf,
    max_items: usize,           // NEW: Enforce limit
    lru_order: VecDeque<String>, // NEW: Track access order
}

impl FeedCache {
    fn evict_if_needed(&mut self) {
        while self.lru_order.len() > self.max_items {
            if let Some(oldest) = self.lru_order.pop_front() {
                let _ = std::fs::remove_file(self.dir.join(&oldest));
            }
        }
    }
}
```

#### 3.4 Add Patent Intelligence Domain

```rust
// src/tools/patents.rs
pub struct PatentSearchInput {
    pub query: String,
    pub office: Option<String>,  // "USPTO", "EPO", "WIPO"
    pub years_back: Option<u32>,
}

// APIs: USPTO Open Data Portal (free), Google Patents (scraping)
// New tools: patents.search, patents.details
```

#### 3.5 Add Government/Legal Intelligence Domain

```rust
// src/tools/govt.rs
pub struct BillSearchInput {
    pub query: String,
    pub congress: Option<u32>,
    pub status: Option<String>,
}

// APIs: congress.gov (free), federalregister.gov (free)
// New tools: govt.bills, govt.regulations
```

#### 3.6 Implement SOP Chains (ANX-Inspired)

```rust
// Multi-step intelligence workflows as composable chains
pub struct IntelligenceSop {
    pub name: String,
    pub steps: Vec<SopStep>,
}

pub struct SopStep {
    pub tool: String,
    pub params: serde_json::Value,
    pub depends_on: Option<usize>,  // Step index this depends on
}

// Example: Deep Research SOP
let deep_research = IntelligenceSop {
    name: "Deep Research Pipeline".to_string(),
    steps: vec![
        SopStep { tool: "web.search", params: json!({"query": "$QUERY"}), depends_on: None },
        SopStep { tool: "web.scrape", params: json!({"url": "$TOP_RESULT}"), depends_on: Some(0) },
        SopStep { tool: "news.enrich", params: json!({"items": "$SCRAPED}"), depends_on: Some(1) },
        SopStep { tool: "insights.indexArticles", params: json!({"articles": "$ENRICHED}"), depends_on: Some(2) },
    ],
};
```

**Deliverables**:
- [ ] InsightStorage with indexed lookups (entity, domain, time)
- [ ] Batch SQLite writes with transactions
- [ ] LRU cache eviction for FeedCache
- [ ] Patent domain (2 tools + USPTO integration)
- [ ] Government domain (2 tools + congress.gov integration)
- [ ] SOP chain executor for multi-step workflows
- [ ] Load testing: validate 10K+ article performance

---

## 5. New Intelligence Domains — Prioritized Roadmap

### Tier 1: High-Value, Easy Integration (Phase 2)

| Domain | Tools | API | Cost | Intelligence Value |
|--------|-------|-----|------|-------------------|
| **Weather** | `weather.forecast`, `weather.alerts`, `weather.current` | OpenWeatherMap | Free (1000/day) | Real-time environmental intel |
| **Financial** | `finance.market`, `finance.crypto`, `finance.trending` | Yahoo Finance, CoinGecko | Free | Economic intelligence |
| **CVE/Security** | `security.cve`, `security.advisories` | NVD | Free | Threat intelligence |

### Tier 2: Medium Complexity (Phase 3)

| Domain | Tools | API | Cost | Intelligence Value |
|--------|-------|-----|------|-------------------|
| **Patents** | `patents.search`, `patents.details` | USPTO Open Data | Free | Innovation tracking |
| **Government** | `govt.bills`, `govt.regulations` | congress.gov, federalregister.gov | Free | Policy tracking |
| **Court Records** | `court.filings` | PACER | Paid | Legal intelligence |

### Tier 3: Advanced Capabilities (Future)

| Domain | Tools | API | Cost | Intelligence Value |
|--------|-------|-----|------|-------------------|
| **Satellite/Imagery** | `geo.satellite`, `geo.fires` | Sentinel Hub, NASA FIRMS | Free/Paid | Environmental monitoring |
| **Supply Chain** | `supply.vessels`, `supply.trade` | MarineTraffic, UN Comtrade | Paid | Logistics intelligence |
| **Election/Political** | `political.candidates`, `political.funding` | OpenSecrets, FEC | Free | Political intelligence |

---

## 6. Implementation Guidelines

### 6.1 Adding a New Tool (Checklist)

```
1. Define Input/Output types in src/tools/types.rs
   - Derive Debug, Serialize, Deserialize, JsonSchema
   - Embed OutputOptions via #[serde(flatten)]
   
2. Implement handler in src/tools/<domain>.rs
   - Use shared HttpClient from server state
   - Return Result<T, IgsError> (not String)
   - Use format_output() helper
   
3. Register in src/server.rs
   - Add #[tool(name = "...", description = "...")] method
   - Use format_output() for consistent output
   
4. Add to src/tools/registry.rs
   - Add tool name to appropriate ToolGroup
   
5. Add to src/tools/mod.rs
   - Add pub mod <domain>;
   
6. Update tool_guide (if applicable)
   - Add to decision_tree and categories
   
7. Write tests
   - Unit test for parser/implementation
   - Integration test for tool registration
```

### 6.2 Tool Naming Conventions

```
Pattern: <domain>.<action>
Examples:
  news.fetch, news.enrich, news.testSource
  research.search, research.paper, research.download
  web.search, web.scrape, web.crawl, web.map
  insights.findConnections, insights.trendingEntities
  
New domains:
  weather.forecast, weather.alerts, weather.current
  finance.market, finance.crypto, finance.trending
  security.cve, security.advisories
  patents.search, patents.details
  govt.bills, govt.regulations
```

### 6.3 Description Budget

Each tool description should be **1 sentence, 20-50 tokens max**.

```
BAD:  "This tool allows you to fetch the latest weather forecast for a given location..."
GOOD: "Fetch weather forecast for a location. Returns daily forecasts and alerts."

BAD:  "Search for academic papers across multiple databases including arXiv and Semantic Scholar..."
GOOD: "Search arXiv and Semantic Scholar for academic papers."
```

---

## 7. Success Metrics

### Phase 1 Metrics

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| HttpClient creations/session | 16 | 1 | Count `HttpClient::new()` calls |
| Settings loads/session | 23+ | 1 | Count `config::load_settings()` calls |
| Format boilerplate lines | ~200 | 0 | `wc -l server.rs` |
| Context overhead (all tools) | ~28% | ~20% | Token count of tool definitions |

### Phase 2 Metrics

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| Tool discovery time | Manual | 1 call | `tool_guide` invocation count |
| Error actionable feedback | 0% | 100% | All errors include suggestion |
| Intelligence domains | 7 | 10 | Count tool groups |
| Total tools | 41 | 50+ | `total_tool_count()` |

### Phase 3 Metrics

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| InsightStorage lookup | O(n) | O(1) | Entity lookup benchmark |
| Cache eviction | None | LRU | Cache size monitoring |
| SQLite write batch | N queries | 1 transaction | Query count per index call |
| 10K article performance | Degrades | Stable | Benchmark with 10K articles |

---

## 8. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing tool contracts | Medium | High | Maintain backward compatibility, version tools |
| API rate limiting (new domains) | High | Medium | Implement per-domain rate limiting, cache aggressively |
| Context overhead increase | Medium | Medium | Monitor token usage, enforce description budget |
| Lightpanda binary compatibility | Low | High | Pin version, test across platforms |
| SQLite migration issues | Low | Medium | Use migrations, test rollback paths |

---

## 9. Dependencies

### New Crates (Phase 1-3)

| Crate | Purpose | Phase |
|-------|---------|-------|
| `lru` | LRU cache eviction for FeedCache | 3 |
| `chrono` | Time-based indexing for InsightStorage | 3 |
| `uuid` | Unique IDs for SOP chains | 2 |

### External APIs (Phase 2-3)

| API | Key Required | Free Tier | Phase |
|-----|--------------|-----------|-------|
| OpenWeatherMap | Yes | 1000 calls/day | 2 |
| Yahoo Finance | No | Unlimited | 2 |
| CoinGecko | No | 30 calls/min | 2 |
| NVD | No | 50 calls/30s | 2 |
| USPTO Open Data | No | Unlimited | 3 |
| congress.gov | No | Unlimited | 3 |

---

## 10. Conclusion

This upgrade plan transforms IGS from a functional tool collection into a protocol-first, agent-native intelligence platform. By applying ANX's principles of progressive disclosure, semantic precision, and security-by-default, combined with production-validated MCP patterns like the `tool_guide` meta-tool, we achieve:

1. **47-57% token reduction** (via tool_guide + progressive loading)
2. **Zero redundant I/O** (via shared state architecture)
3. **10+ intelligence domains** (weather, finance, security, patents, government)
4. **Scalable to 10K+ articles** (via indexed storage + batch writes)
5. **Production-ready error handling** (via structured IgsError type)

The three-phase approach ensures each increment is independently valuable and deployable, with clear success metrics and risk mitigation.

---

## Appendix A: File Reference

| File | Lines | Purpose |
|------|-------|---------|
| `src/server.rs` | 836 | MCP server, tool router, format output |
| `src/tools/registry.rs` | 101 | Tool group definitions |
| `src/tools/types.rs` | 884 | All tool I/O types |
| `src/tools/types_base.rs` | ~100 | Shared base types |
| `src/tools/news.rs` | ~400 | News fetch/enrich/pipeline |
| `src/tools/web.rs` | 727 | Web search/scrape/crawl/map |
| `src/tools/insights.rs` | ~200 | Cross-article analysis |
| `src/tools/lp_mcp.rs` | ~400 | Lightpanda browser tools |
| `src/http.rs` | ~300 | HTTP client with caching |
| `src/cache.rs` | ~150 | Dual-tier caching |
| `src/config.rs` | ~200 | YAML config loading |
| `src/parsers.rs` | 1389 | 11 parser implementations |
| `src/types.rs` | 547 | Shared domain types |
| `src/error.rs` | (new) | Structured error types |

---

## Appendix B: Tool Guide Template

```json
{
  "decision_tree": {
    "I need current news": "news.fetch",
    "I need to scrape a website": "web.scrape",
    "I need academic papers": "research.search",
    "I need to monitor Reddit": "reddit.search",
    "I need weather data": "weather.forecast",
    "I need market data": "finance.market",
    "I need vulnerability info": "security.cve",
    "I need patent search": "patents.search",
    "I need government bills": "govt.bills",
    "I need cross-source analysis": "insights.findConnections",
    "I need to browse JS pages": "lightpanda.goto"
  },
  "categories": {
    "Discovery": ["pools.list", "sources.list", "parsers.list"],
    "News": ["news.fetch", "news.enrich", "news.testSource"],
    "Research": ["research.search", "research.paper", "research.download"],
    "Web": ["web.search", "web.scrape", "web.crawl", "web.map"],
    "Insights": ["insights.findConnections", "insights.trendingEntities"],
    "Social": ["reddit.search", "reddit.feed"],
    "Weather": ["weather.forecast", "weather.alerts", "weather.current"],
    "Finance": ["finance.market", "finance.crypto", "finance.trending"],
    "Security": ["security.cve", "security.advisories"],
    "Browser": ["lightpanda.goto", "lightpanda.markdown", "..."]
  },
  "drill_down_chains": [
    {
      "name": "Deep Research",
      "steps": ["web.search", "web.scrape", "news.enrich", "insights.indexArticles"]
    },
    {
      "name": "Threat Monitoring",
      "steps": ["security.cve", "news.fetch", "insights.findConnections"]
    }
  ]
}
```
