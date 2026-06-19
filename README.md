# IGS — Intelligence Gathering System

[![GitHub](https://img.shields.io/badge/GitHub-ishan--parihar/igs--rust--mcp-181717?logo=github)](https://github.com/ishan-parihar/igs-rust-mcp)
[![GitLab](https://img.shields.io/badge/GitLab-ishan--parihar/igs--rust--mcp-FC6D26?logo=gitlab)](https://gitlab.com/ishan-parihar/igs-rust-mcp)

MCP server + CLI for intelligence gathering. 50 tools, 411 sources, 47 countries, [TOON](https://toonformat.dev) token-efficient output, Lightpanda headless browser.

| Metric | Value |
|--------|-------|
| Tools | 50 (29 core + 21 Lightpanda browser automation) |
| Sources | 411 across 47 countries |
| Pools | 18 (geopolitics, tech, India, defense, health, etc.) |
| Binary | Single `igs` binary (~19 MB musl static) |
| Output | TOON (default, ~40% fewer tokens) or JSON |

---

## Installation

### Option 1: Download Release

```bash
# Download latest release
curl -L -o igs.tar.gz https://github.com/ishan-parihar/igs-rust-mcp/releases/latest/download/igs-v0.4.0-x86_64-linux-musl.tar.gz

# Extract
tar -xzf igs.tar.gz

# Move to PATH
sudo mv igs /usr/local/bin/
sudo ln -sf /usr/local/bin/igs /usr/local/bin/igs-mcp  # backward compat

# Verify
igs --version
igs status
```

### Option 2: Install Script

```bash
curl -sSL https://raw.githubusercontent.com/ishan-parihar/igs-rust-mcp/master/scripts/install.sh | bash
```

### Option 3: Build from Source

```bash
git clone https://github.com/ishan-parihar/igs-rust-mcp.git
cd igs-rust-mcp
cargo build --release
./target/release/igs status
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

### As CLI

```bash
# System status
igs status

# Fetch news
igs news fetch --pools GLOBAL_TECH_CYBER --limit 10

# Search Reddit
igs reddit search --query "AI safety"

# Search academic papers
igs research search --query "transformer architecture"

# Web search (requires Tavily API key)
igs web search --query "rust async runtime"

# Scrape a URL to markdown
igs web scrape --url https://example.com

# Crawl a website (requires Lightpanda enabled)
igs web crawl --url https://example.com --max-depth 2

# Browser automation (requires Lightpanda enabled)
igs browser goto --url https://example.com
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
├── pools.yml         # 18 pool definitions
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
  userAgent: IGS-MCP/0.1
  timeoutMs: 15000
  retries: 2
  concurrency: 6
  perHost: 2

# Feed caching
cache:
  enabled: true
  ttlMs: 1800000        # 30 minutes
  queryTtlMs: 600000    # 10 minutes

# Web search (requires API key)
tavily:
  enabled: false
  apiKey: ${TAVILY_API_KEY}

firecrawl:
  enabled: false
  apiKey: ${FIRECRAWL_API_KEY}

# Lightpanda headless browser (auto-downloads binary)
lightpanda:
  enabled: false
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
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `IGS_CONFIG_DIR` | `~/.config/igs-mcp/` | Config directory |
| `RUST_LOG` | `info` | Log level (`debug`, `trace`) |
| `TAVILY_API_KEY` | — | Tavily web search API key |
| `FIRECRAWL_API_KEY` | — | Firecrawl API key |

---

## Tools

### Core Tools (29)

| Domain | Tools | Description |
|--------|-------|-------------|
| **Pools** | `pools.list`, `pools.upsert`, `pools.delete` | Manage source groupings |
| **Sources** | `sources.list/upsert/delete`, `sources.autodiscover`, `sources.enableGenericScraper`, `sources.countries`, `sources.cities`, `sources.domains` | CRUD + auto-discovery + geo |
| **Parsers** | `parsers.list` | List available parser keys |
| **News** | `news.fetch`, `news.testSource`, `news.enrich` | Fetch, test, NLP enrichment |
| **Reddit** | `reddit.search`, `reddit.feed` | Search Reddit posts, follow subreddit feeds |
| **Research** | `research.search`, `research.paper`, `research.download` | arXiv + Semantic Scholar |
| **Web** | `web.search`, `web.scrape`, `web.crawl`, `web.map` | Search, scrape, crawl, sitemap |
| **Insights** | `insights.findConnections`, `insights.trendingEntities`, `insights.indexArticles`, `insights.getStats`, `insights.clearIndex` | Cross-article analysis |

### Lightpanda Browser Tools (12)

| Tool | Description |
|------|-------------|
| `lightpanda.goto` | Navigate to URL (JS rendering) |
| `lightpanda.markdown` | Get page as markdown |
| `lightpanda.links` | Extract links |
| `lightpanda.evaluate` | Execute JavaScript |
| `lightpanda.semantic_tree` | AI-friendly DOM tree |
| `lightpanda.structuredData` | Extract JSON-LD, OpenGraph |
| `lightpanda.detectForms` | Find forms |
| `lightpanda.click` | Click element |
| `lightpanda.fill` | Fill form field |
| `lightpanda.scroll` | Scroll page |
| `lightpanda.waitForSelector` | Wait for element |
| `lightpanda.interactiveElements` | Find clickable items |

### CLI Browser Commands

```bash
igs browser goto --url https://example.com
igs browser markdown
igs browser links
igs browser evaluate --expression "document.title"
igs browser semantic-tree --include-text
igs browser structured-data
igs browser detect-forms
igs browser click --selector "button.submit"
igs browser fill --selector "input[name=email]" --value "user@example.com"
igs browser scroll --direction down --pixels 500
igs browser wait-for-selector --selector ".content" --timeout-ms 5000
igs browser interactive-elements
```

---

## Lightpanda Integration

IGS uses [Lightpanda](https://github.com/lightpanda-io/browser) in two ways:

### Level 1: CLI Subprocess (web.scrape, web.crawl)

Stateless — fetches a single page via `lightpanda fetch --dump markdown`. Used by `web.scrape` with `provider: "lightpanda"` and `web.crawl`.

```bash
# Scrape with JS rendering
igs web scrape --url https://spa-site.com --provider lightpanda

# Crawl with BFS
igs web crawl --url https://example.com --max-depth 2 --max-pages 20
```

### Level 2: MCP Sub-Server (lightpanda.* tools)

Stateful — spawns `lightpanda mcp` as a persistent subprocess. The page stays loaded between calls. Supports JavaScript execution, form interaction, navigation.

```bash
# Navigate and extract
igs browser goto --url https://example.com
igs browser markdown
igs browser evaluate --expression "document.querySelectorAll('h1').length"

# Form interaction
igs browser goto --url https://example.com/login
igs browser detect-forms
igs browser fill --selector "input[name=username]" --value "admin"
igs browser click --selector "button[type=submit]" --wait-for-navigation true
igs browser markdown
```

The Lightpanda binary auto-downloads to `~/.config/igs-mcp/bin/` and checks for updates daily.

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
├── cache.rs             Dual-tier caching
├── parsers.rs           7 parser types + filtering + dedup
├── lightpanda.rs        Lightpanda binary manager
├── lightpanda_mcp.rs    Lightpanda MCP client (JSON-RPC 2.0)
├── persistence.rs       SQLite persistence
└── tools/
    ├── types.rs         All tool I/O types
    ├── helpers.rs       NLP, urlencoding, toon_encode
    ├── pools.rs         Pool CRUD
    ├── sources.rs       Source CRUD + autodiscover + geo
    ├── parsers.rs       Parser listing
    ├── news.rs          News fetch + enrichment
    ├── reddit.rs        Reddit search
    ├── research.rs      Academic papers
    ├── web.rs           Web search/scrape/crawl/map
    ├── insights.rs      Cross-article analysis
    ├── intelligence.rs  Pipeline
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
