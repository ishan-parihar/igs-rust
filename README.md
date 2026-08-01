# IGS — Intelligence Gathering System v1.0.0

[![GitHub](https://img.shields.io/badge/GitHub-ishan--parihar/igs--rust-181717?logo=github)](https://github.com/ishan-parihar/igs-rust)
[![GitLab](https://img.shields.io/badge/GitLab-ishan--parihar/igs--rust-FC6D26?logo=gitlab)](https://gitlab.com/ishan-parihar/igs-rust)

MCP server + CLI for intelligence gathering. 64 tools, 411 sources, 47 countries, [TOON](https://toonformat.dev) token-efficient output, Obscura headless browser.

| Metric | Value |
|--------|-------|
| Tools | 64 (56 core + 8 Obscura browser) |
| Intelligence Domains | 20 (Discovery, News, Research, Web, Insights, Social, Twitter, Weather, Finance, Security, Patents, Government, Legal, Environment, Climate, Health, Politics, Browser, SOP, YouTube) |
| Sources | 411 across 47 countries |
| Pools | 14 (geopolitics, tech, India, defense, health, etc.) |
| Binary | Single `igs` binary (~26 MB musl static) |
| Output | TOON (default, ~40% fewer tokens) or JSON |
| API Keys | **None required** — all web search uses Obscura + DuckDuckGo |
---

## Installation

### Option 1: Install Script (Recommended)

Detects your platform, downloads the latest release, and installs to `~/.local/bin`:

```bash
curl -sSL https://raw.githubusercontent.com/ishan-parihar/igs-rust/master/scripts/install.sh | bash
```

### Option 2: Manual Download

Download the tarball for your platform from the [latest release](https://github.com/ishan-parihar/igs-rust/releases/latest), then:

```bash
tar -xzf igs-*.tar.gz
sudo mv igs /usr/local/bin/
igs --version
```

### Option 3: Build from Source

```bash
# Prerequisites
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add x86_64-unknown-linux-musl

# Clone and build
git clone https://github.com/ishan-parihar/igs-rust.git
cd igs-rust
cargo build --release --target x86_64-unknown-linux-musl

# Install
sudo cp target/x86_64-unknown-linux-musl/release/igs /usr/local/bin/
igs --version
```

---

## Quick Start

### As MCP Server (for AI agents)

```bash
# Start the MCP server on stdio
igs mcp
```

Configure in **Claude Desktop** (`~/.config/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "igs": {
      "command": "igs",
      "args": ["mcp"]
    }
  }
}
```

Configure in **Cursor** (`.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "igs": {
      "command": "igs",
      "args": ["mcp"]
    }
  }
}
```

Configure in **OpenCode** (`~/.config/opencode/opencode.json`):

```json
{
  "mcp": {
    "igs": {
      "type": "local",
      "command": ["/usr/local/bin/igs", "mcp"],
      "enabled": true
    }
  }
}
```

### As CLI

```bash
# System status
igs status

# Fetch news
igs news fetch --pools GLOBAL_TECH_CYBER --limit 10

# Search Reddit
igs reddit search --query "AI safety"

# Search academic papers
# Search web (zero API keys — uses DuckDuckGo via Obscura)
igs web search --query "rust async runtime"

# Scrape a URL to markdown
igs web scrape --url https://example.com

# Crawl a website (requires Obscura enabled)
igs web crawl --url https://example.com --max-depth 2

# Browser automation (requires Obscura enabled)
igs browser goto --url https://example.com
igs browser markdown
igs browser links
igs browser markdown
igs browser links

# List available pools, sources, parsers
igs pools list
igs sources list --pool GLOBAL_TECH_CYBER
igs sources countries
igs parsers
```

### Output Format

All bulk data tools default to [TOON](https://toonformat.dev) (token-efficient). Use `--format json` for standard JSON:

```bash
igs --format json news fetch --pools GLOBAL_TECH_CYBER --limit 5
igs --format toon news fetch --pools GLOBAL_TECH_CYBER --limit 5
```

---

## Configuration

### Config Directory

IGS auto-creates `~/.config/igs-mcp/` on first run with default config files:

```
~/.config/igs-mcp/
├── settings.yml      # Main configuration
├── pools.yml         # 14 pool definitions
├── sources.yml       # 411 source definitions
├── countries.yml     # 47 country metadata
├── insights.db       # SQLite database (auto-created)
├── cache/            # Feed cache (auto-managed)
└── bin/              # Lightpanda binary (auto-downloaded)
```

Override with: `export IGS_CONFIG_DIR=/path/to/config`

### settings.yml

```yaml
# HTTP client
http:
  userAgent: IGS/1.0.0 (+https://github.com/ishan-parihar/igs-rust)
  timeoutMs: 15000
  retries: 2
  concurrency: 6
  perHost: 2

# Feed caching
cache:
  enabled: true
  ttlMs: 1800000        # 30 minutes
  queryTtlMs: 600000    # 10 minutes

# Obscura headless browser (auto-downloads binary)
# Powers: web.search (via DDG), web.scrape (JS rendering), web.crawl, browser.*
browser:
  enabled: true
  default: obscura
  auto_update: true
  obey_robots: true
  timeout_ms: 30000
  max_concurrent: 10

# NLP enrichment (offline, no API calls)
nlp:
  enabled: true
  max_topics: 8
  max_entities: 20
  dedup_threshold: 0.3

# Intelligence pipeline
pipeline:
  default_pool: GLOBAL_TECH_CYBER
  default_limit: 50
  persist_insights: true

# Output format
output:
  default_format: toon  # "toon" or "json"

# API Keys (optional — tools work without them)
openweather:
  enabled: false
  apiKey: ${OPENWEATHER_API_KEY}

noaa:
  enabled: false
  apiKey: ${NOAA_API_KEY}

courtlistener:
  enabled: false
  apiKey: ${COURTLISTENER_API_KEY}
```
# NLP enrichment (offline, no API calls)
nlp:
  enabled: true
  max_topics: 8
  max_entities: 20
  dedup_threshold: 0.3

# Intelligence pipeline
pipeline:
  default_pool: GLOBAL_TECH_CYBER
  default_limit: 50
  persist_insights: true

# Output format
output:
  default_format: toon  # "toon" or "json"

# API Keys (optional - tools work without them but with rate limits)
openweather:
  enabled: false
  apiKey: ${OPENWEATHER_API_KEY}

noaa:
  enabled: false
  apiKey: ${NOAA_API_KEY}

courtlistener:
  enabled: false
  apiKey: ${COURTLISTENER_API_KEY}
```

| Variable | Default | Description |
|----------|---------|-------------|
| `IGS_CONFIG_DIR` | `~/.config/igs-mcp/` | Config directory |
| `RUST_LOG` | `info` | Log level (`debug`, `trace`) |
| `OPENWEATHER_API_KEY` | — | OpenWeatherMap API key (free tier: 1000/day) |
| `NOAA_API_KEY` | — | NOAA Climate Data Online API key (free) |
| `COURTLISTENER_API_KEY` | — | CourtListener API token (free) |

---

## Tools (68 Total)

### Discovery (13 tools)

| Tool | Description |
|------|-------------|
| `pools.list` | List source pools |
| `pools.upsert` | Create/update a pool |
| `pools.delete` | Delete a pool |
| `sources.list` | List news sources |
| `sources.upsert` | Create/update a source |
| `sources.delete` | Delete a source |
| `sources.autodiscover` | Auto-discover RSS feeds |
| `sources.enable_generic_scraper` | Enable HTML scraping |
| `sources.countries` | List countries with source counts |
| `sources.cities` | List cities with source counts |
| `sources.domains` | List domains with source counts |
| `parsers.list` | List available parser types |
| `igs://tool-guide` (MCP resource) | Decision tree for tool selection |

### News (3 tools)

| Tool | Description |
|------|-------------|
| `news.fetch` | Fetch news from sources (depth=deep for full pipeline) |
| `news.test_source` | Test a single source |
| `news.enrich` | NLP enrichment (topics, entities, sentiment) |

### Research (4 tools)

| Tool | Description |
|------|-------------|
| `research.search` | Search arXiv + Semantic Scholar |
| `research.paper` | Get paper details with citations |
| `research.download` | Download paper PDF |
| `research.pubmed_search` | Search PubMed medical research |

### Web (4 tools)

| Tool | Description |
| `web.search` | Real-time web search (DDG + Wikipedia + GitHub + HN + StackOverflow) |
| `web.scrape` | Scrape URL to markdown (HTTP or Obscura JS rendering) |
| `web.crawl` | BFS crawl website via Obscura |
| `web.map` | Discover URLs from sitemap |

### Insights (5 tools)

| Tool | Description |
|------|-------------|
| `insights.find_connections` | Find cross-domain connections |
| `insights.trending_entities` | Detect trending entities |
| `insights.index_articles` | Index articles for analysis |
| `insights.get_stats` | Engine statistics |
| `insights.clear_index` | Clear all indexed articles |

### Social (2 tools)

| Tool | Description |
|------|-------------|
| `reddit.search` | Search Reddit posts |
| `reddit.feed` | Follow subreddit feeds |

### Weather (3 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `weather.forecast` | 5-day forecast | Required |
| `weather.current` | Current conditions | Required |
| `weather.alerts` | Severe weather alerts | Required |

### Finance (3 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `finance.market` | Stock market quotes | Not required |
| `finance.crypto` | Cryptocurrency prices | Not required |
| `finance.trending` | Trending cryptocurrencies | Not required |

### Security (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `security.cve` | Search CVE vulnerabilities | Not required |
| `security.advisories` | Search GitHub advisories | Not required |

### Patents (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `patents.search` | Search USPTO patents | Not required |
| `patents.details` | Get patent details | Not required |

### Government (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `govt.bills` | Search congressional bills | Not required (DEMO_KEY) |
| `govt.regulations` | Search federal regulations | Not required |

### Politics (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `politics.fec_candidates` | Search FEC candidates | Not required (optional) |
| `politics.fec_committees` | Search FEC committees | Not required (optional) |

### Health (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `health.cdc_leading_causes` | Leading causes of death (US) | Not required |
| `health.who_gho` | Global health indicators (194 countries) | Not required |

### Environment (3 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `env.epa_facilities` | EPA-regulated facilities | Not required |
| `env.epa_emissions` | Toxic release inventory | Not required |
| `satellite.firms_fires` | NASA FIRMS fire hotspots | Not required (DEMO_KEY) |

### Climate (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `climate.noaa_observations` | Historical weather observations | Required |
| `climate.noaa_stations` | Find weather stations | Required |

### Legal (2 tools)

| Tool | Description | API Key |
|------|-------------|---------|
| `legal.search_cases` | Search case law | Required |
| `legal.case_details` | Get case details | Required |

### SOP (2 tools)

| Tool | Description |
|------|-------------|
| `sop.list` | List available workflows |
| `sop.execute` | Execute multi-step workflow |

### Browser (8 tools)

| Tool | Description |
|------|-------------|
| `browser.goto` | Navigate to URL (JS rendering) |
| `browser.markdown` | Get page as markdown |
| `browser.links` | Extract links |
| `browser.evaluate` | Execute JavaScript |
| `` | AI-friendly DOM tree |
| `` | Extract JSON-LD, OpenGraph |
| `` | Find forms |
| `browser.click` | Click element |
| `browser.fill` | Fill form field |
| `browser.scroll` | Scroll page |
| `browser.wait_for_selector` | Wait for element |
| `` | Find clickable items |

---

## Dependencies

### System Requirements

- **OS**: Linux (x86_64), macOS, or WSL2
- **Memory**: 50 MB minimum
- **Disk**: 100 MB for binary + config
- **Network**: Required for API calls

### Rust Dependencies (for building from source)

| Crate | Purpose |
|-------|---------|
| `rmcp` | MCP protocol implementation |
| `reqwest` | HTTP client |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `serde_yaml` | YAML config parsing |
| `clap` | CLI argument parsing |
| `chrono` | Date/time handling |
| `rusqlite` | SQLite persistence |
| `url` | URL parsing and encoding |
| `feed-rs` | RSS/Atom feed parsing |
| `scraper` | HTML parsing |
| `toon_format` | Token-efficient output |
| `tracing` | Logging |

### External APIs (Optional)

| API | Purpose | Free Tier | Key Required |
|-----|---------|-----------|--------------|
| OpenWeatherMap | Weather data | 1000 calls/day | Yes |
| NOAA CDO | Climate data | 10,000 req/day | Yes |
| CourtListener | Case law | 125 req/day | Yes |
| DuckDuckGo | Web search | Unlimited | No |
| Obscura | JS rendering | Unlimited | No |
| Yahoo Finance | Stock quotes | Unlimited | No |
| CoinGecko | Crypto prices | 30 req/min | No |
| NVD | CVE vulnerabilities | Rate-limited | No |
| GitHub Advisory | Security advisories | Unlimited | No |
| PatentsView | Patent search | Unlimited | No |
| Congress.gov | Bills/regulations | 40 req/hour | No (DEMO_KEY) |
| Federal Register | Regulations | Unlimited | No |
| CDC SODA | Health statistics | 1000 req/hour | No |
| WHO GHO | Global health | Unlimited | No |
| NASA FIRMS | Fire detection | Unlimited | No (DEMO_KEY) |
| EPA Envirofacts | Environmental data | Unlimited | No |

---

## Implementation Guide

### Adding a New Intelligence Domain

1. **Create module**: `src/tools/<domain>.rs`
2. **Add types**: `src/tools/types.rs`
3. **Register module**: `src/tools/mod.rs`
4. **Add to registry**: `src/tools/registry.rs`
5. **Add handlers**: `src/server.rs`
6. **Update tool_guide**: `src/tools/tool_guide.rs`
7. **Add CLI commands**: `src/cli.rs` (optional)

### Standard Tool Pattern

```rust
use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use super::types::*;

pub async fn <domain>_<tool>(input: <Domain>Input) -> Result<Domain>Output, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    
    let query = urlencoding(&input.query);
    let url = format!("https://api.example.com/endpoint?q={}", query);
    
    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("API error: {}", e))?;
    
    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("API returned cached response".into())};
    
    let data: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    
    // Parse and return
    Ok(DomainOutput { /* ... */ })
}
```

---

## Architecture

```
src/
├── cli.rs               Single binary entry point (clap + MCP server)
├── lib.rs               Module declarations
├── server.rs            IgsMcpServer, tool router, InsightStorage (SQLite)
├── config.rs            YAML config loading
├── types.rs             Shared types (Settings, NewsItem, etc.)
├── http.rs              HttpClient with retry, backoff, per-host concurrency
├── cache.rs             Dual-tier caching with LRU eviction
├── parsers.rs           11 parser types + filtering + dedup
├── obscura.rs        Obscura binary manager (download, update, fetch)
├── lp_mcp.rs         Obscura MCP client (CLI subprocess invocation)
├── persistence.rs       SQLite persistence
└── tools/
    ├── types.rs         All tool I/O types (64 tools)
    ├── tool_guide.rs    Decision tree + categories + drill-down chains
    ├── helpers.rs       NLP, urlencoding, toon_encode
    ├── pools.rs         Pool CRUD
    ├── sources.rs       Source CRUD + autodiscover + geo
    ├── parsers.rs       Parser listing
    ├── news.rs          News fetch + enrichment
    ├── reddit.rs        Reddit search
    ├── research.rs      Academic papers + PubMed
    ├── web.rs           Web search/scrape/crawl/map
    ├── insights.rs      Cross-article analysis
    ├── weather.rs       OpenWeatherMap integration
    ├── finance.rs       Yahoo Finance + CoinGecko
    ├── security.rs      NVD + GitHub Advisory
    ├── patents.rs       PatentsView API
    ├── govt.rs          Congress.gov + Federal Register
    ├── politics.rs      FEC API
    ├── health.rs        CDC + WHO GHO
    ├── climate.rs       NOAA CDO
    ├── legal.rs         CourtListener
    ├── env.rs           EPA Envirofacts + NASA FIRMS
    ├── sop.rs           Multi-step workflows
    └── lp_mcp.rs        Lightpanda MCP tool wrappers
```

---

## Docker

```bash
docker build -t igs .
docker run -v ~/.config/igs-mcp:/root/.config/igs -e IGS_CONFIG_DIR=/root/.config/igs igs mcp
```

---

## License

MIT

---

Developed by [Ishan Parihar](https://github.com/ishanparihar)

---

## Agent Integration (AXI §7)

IGS ships an installable AI agent skill that provides ambient context at session start — showing tool counts, pool status, and contextual help hints.

### Install the Skill

```bash
# Via npx (recommended)
npx skills add ishan-parihar/igs-rust --skill igs

# Or download manually
curl -fsSL https://raw.githubusercontent.com/ishan-parihar/igs-rust/master/SKILL.md \
  -o ~/.agents/skills/igs/SKILL.md
```

### Session Hook (Claude Code)

Add to `~/.claude/settings.json` or project `.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "igs"
          }
        ]
      }
    ]
  }
}
```

At session start, IGS prints a compact dashboard:

```
bin: ~/.local/bin/igs
description: Intelligence Gathering System — 64 tools, 411 sources, 47 countries

pools[14]{name,description}:
  GLOBAL_TECH_CYBER,Cybersecurity and tech news worldwide
  ...

total_sources: 411
total_countries: 47

help[4]:
  Run `igs news fetch --pools GLOBAL_TECH_CYBER --limit 10` for recent tech news
  Run `igs research search --query "topic"` for academic papers
  Run `igs web search --query "topic"` for web search
  Run `igs pools list` to see all 14 intelligence pools
```

### Session Hook (Codex)

Add to `~/.codex/hooks.json` or project `.codex/hooks.json`:

```json
{
  "SessionStart": "igs"
}
```

Ensure hooks are enabled in `~/.codex/config.toml`:

```toml
[features]
hooks = true
```

### Session Hook (OpenCode)

Create `~/.config/opencode/plugins/igs.ts`:

```typescript
export default {
  name: "igs",
  onSessionStart: async () => {
    const { execSync } = require("child_process");
    return execSync("igs").toString();
  },
};
```
