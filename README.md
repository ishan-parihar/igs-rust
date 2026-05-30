# IGS MCP Server (Rust)

[![GitHub](https://img.shields.io/badge/GitHub-ishan--parihar/igs--rust--mcp-181717?logo=github)](https://github.com/ishan-parihar/igs-rust-mcp)
[![GitLab](https://img.shields.io/badge/GitLab-ishan--parihar/igs--rust--mcp-FC6D26?logo=gitlab)](https://gitlab.com/ishan-parihar/igs-rust-mcp)

**Intelligence Gathering System** — Rust MCP server with 30 tools, 411 sources, 47 countries, [TOON](https://toonformat.dev) token-efficient output, Lightpanda headless browser integration, and a CLI.

## Overview

IGS monitors intelligence from 411 curated sources across global news, geopolitics, tech, research, and regional topics. It provides both an MCP server (for AI agents) and a CLI (for direct use).

| Metric | Value |
|--------|-------|
| Tools | 30 (pools, sources, news, reddit, research, web, insights, intelligence) |
| Sources | 411 across 47 countries |
| Pools | 18 (geopolitics, tech, India, defense, health, etc.) |
| Parsers | 7 (rss, ofac, who_dons, newslaundry, semantic_scholar, generic_html, auto-detect) |
| Binary | ~7 MB (release-stripped), ~5 MB RSS idle |
| Output | TOON (default, ~40-60% fewer tokens) or JSON |

## The Problem

Raw intelligence gathering hits a "token wall." Monitoring hundreds of global sources produces massive unstructured data that exhausts LLM context windows. Naive text extraction destroys structural hierarchy, while standard JSON adds significant token overhead. IGS solves this by delivering structured, token-efficient intelligence to AI agents.

## Features

| Domain | Tools | What it does |
|--------|-------|-------------|
| **Pools** | `pools.list`, `pools.upsert`, `pools.delete` | Manage source groupings (18 pools) |
| **Sources** | `sources.list/upsert/delete`, `sources.autodiscover`, `sources.enableGenericScraper` | CRUD + auto-discovery for 411 sources |
| **Geo** | `sources.countries`, `sources.cities`, `sources.domains` | List countries/cities/domains with source counts |
| **Parsers** | `parsers.list` | List available parser keys (rss, ofac, who_dons, etc.) |
| **News** | `news.fetch`, `news.testSource`, `news.enrich` | Fetch news with pool/country/city/domain/keyword/time filtering. Offline NLP enrichment (topics, entities, sentiment, summary). |
| **Reddit** | `reddit.search` | Search Reddit posts via JSON API |
| **Research** | `research.search`, `research.paper`, `research.download` | arXiv + Semantic Scholar search, paper details, PDF download |
| **Web** | `web.search`, `web.scrape`, `web.crawl`, `web.map` | Tavily/Firecrawl search, HTML→markdown scraping, Lightpanda BFS crawl, sitemap discovery |
| **Insights** | `insights.findConnections`, `insights.findAllConnections`, `insights.trendingEntities`, `insights.indexArticles`, `insights.getStats`, `insights.clearIndex` | Cross-article entity analysis with SQLite persistence |
| **Intelligence** | `intelligence.collect` | Full pipeline: fetch→enrich→index in one call |
| **Lightpanda Browser** | `lightpanda.goto`, `lightpanda.markdown`, `lightpanda.links`, `lightpanda.evaluate`, `lightpanda.semantic_tree`, `lightpanda.structuredData`, `lightpanda.detectForms`, `lightpanda.click`, `lightpanda.fill`, `lightpanda.scroll`, `lightpanda.waitForSelector`, `lightpanda.interactiveElements` | Full browser automation via Lightpanda MCP sub-server. Persistent session, JS execution, form filling, navigation. |

### Token-Efficient Output (TOON)

Bulk data tools default to [TOON](https://toonformat.dev) output — a compact alternative to JSON that reduces token usage by ~40-60%. JSON available via `format: "json"` parameter.

### Lightpanda Headless Browser

IGS has two levels of Lightpanda integration:

**Level 1 — CLI subprocess** (`web.crawl`, `web.scrape`): Fetches pages via `lightpanda fetch --dump markdown`. Stateless, single-page.

**Level 2 — MCP sub-server** (`lightpanda.*` tools): Spawns `lightpanda mcp` as a persistent subprocess. Stateful session — navigate, interact, extract across multiple calls. Supports JavaScript execution, form filling, clicking, scrolling, structured data extraction.

The binary auto-downloads to `~/.config/igs-mcp/bin/` and checks for updates daily.

### Intelligence Pipeline

`intelligence.collect` chains `news.fetch` → `news.enrich` → `insights.indexArticles` in one call. After indexing, use `insights.findConnections` or `insights.trendingEntities` for cross-article analysis.

## Quick Start

### Prerequisites

- Rust 1.75+
- (Optional) Tavily or Firecrawl API keys for web search
- (Optional) Lightpanda enabled for JS-rendered crawling

### Build & Run

```bash
# MCP server
cargo build --release
./target/release/igs-mcp

# CLI
cargo build --release --bin igs
./target/release/igs status
```

### CLI Usage

```bash
igs status                                          # System status
igs pools list                                      # List all pools
igs sources list --pool GLOBAL_TECH_CYBER           # List sources in pool
igs sources countries                               # Countries with source counts
igs news fetch --pools GLOBAL_TECH_CYBER --limit 10 # Fetch news
igs news test --id reuters                          # Test a source
igs reddit search --query "AI safety"               # Reddit search
igs research search --query "transformer"           # Academic papers
igs web search --query "rust async"                 # Web search (Tavily)
igs web scrape --url https://example.com            # Scrape URL to markdown
igs web crawl --url https://example.com --max-depth 2  # BFS crawl (Lightpanda)
igs web map --url https://example.com               # Sitemap discovery
igs parsers                                         # List parser keys

# Output format
igs --format json news fetch --pools GLOBAL_TECH_CYBER --limit 5  # JSON output
igs --format toon news fetch --pools GLOBAL_TECH_CYBER --limit 5  # TOON output (default)
```

### MCP Configuration (Claude Desktop / Cursor)

```json
{
  "mcpServers": {
    "igs-mcp": {
      "command": "/absolute/path/to/igs-rust-mcp/target/release/igs-mcp"
    }
  }
}
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `IGS_CONFIG_DIR` | `~/.config/igs-mcp/` | Config directory override |
| `RUST_LOG` | `info` | Log level (e.g., `debug`, `trace`) |

### Config Files

Auto-bootstrapped from `./config/` to `~/.config/igs-mcp/` on first run:

| File | Purpose |
|------|---------|
| `settings.yml` | HTTP, cache, NLP, pipeline, output, Tavily/Firecrawl, Lightpanda settings |
| `pools.yml` | 18 pool definitions |
| `sources.yml` | 411 source definitions |
| `countries.yml` | 47 country metadata |

### Settings Sections

`settings.yml` has 9 configuration sections:

| Section | Key settings |
|---------|-------------|
| `http` | userAgent, timeoutMs, retries, concurrency, perHost |
| `cache` | enabled, dir, ttlMs, queryTtlMs |
| `time` | timezone |
| `tavily` | enabled, apiKey, searchDepth |
| `firecrawl` | enabled, apiKey |
| `lightpanda` | enabled, auto_update, obey_robots, timeout_ms, proxy, max_concurrent |
| `nlp` | enabled, max_topics, max_entities, dedup_threshold |
| `pipeline` | default_pool, default_limit, persist_insights |
| `output` | default_format (toon/json), toon_indent |

## Architecture

```
src/
├── main.rs            MCP server entry point (rmcp stdio transport)
├── cli.rs             CLI binary (clap-based subcommands)
├── lib.rs             Module declarations
├── server.rs          IgsMcpServer, tool router, InsightStorage (SQLite-backed)
├── config.rs          YAML config loading/saving
├── types.rs           Shared types (Settings, NewsItem, ResearchPaper, etc.)
├── http.rs            HttpClient with retry, exponential backoff, per-host concurrency
├── cache.rs           Dual-tier caching (feed cache + query cache)
├── parsers.rs         7 parser types + keyword/time filtering + dedup
├── lightpanda.rs      Lightpanda binary manager (daily version check, auto-download)
├── persistence.rs     SQLite persistence for InsightStorage
└── tools/
    ├── mod.rs         Module re-exports
    ├── types.rs       All tool I/O types (42 structs, all JsonSchema)
    ├── helpers.rs     urlencoding, NLP (topics/entities/sentiment), toon_encode
    ├── pools.rs       Pool CRUD
    ├── sources.rs     Source CRUD + autodiscover + geo
    ├── parsers.rs     Parser listing
    ├── news.rs        News fetch + enrichment
    ├── reddit.rs      Reddit search
    ├── research.rs    Academic paper search + details + download
    ├── web.rs         Web search/scrape/crawl/map
    ├── insights.rs    Cross-article entity analysis
    └── intelligence.rs Pipeline: fetch→enrich→index
```

## Docker

```bash
docker build -t igs-rust-mcp .
docker run -v ~/.config/igs-mcp:/root/.config/igs-mcp igs-rust-mcp
```

Multi-stage: `rust:1.85-slim-bookworm` builder → `debian:bookworm-slim` runtime. Final image ~15–20 MB.

## Size & Performance

| Metric | Value |
|--------|-------|
| Binary | ~14 MB (debug), ~7 MB (release-stripped) |
| RSS (idle) | ~5 MB |
| Docker image | ~15–20 MB |
| Sources | 411 across 47 countries |
| Pools | 18 |
| Tools | 30 |
| Startup | < 100 ms |

## License

MIT

---

Developed by [Ishan Parihar](https://github.com/ishanparihar)
