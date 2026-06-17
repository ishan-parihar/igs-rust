# AGENTS.md — IGS MCP Server

Guide for AI agents using IGS as an intelligence gathering tool.

## Quick Reference

### First Steps

1. `pools.list` — see available source pools
2. `sources.list` — see available sources
3. `parsers.list` — see available parser types
4. `news.fetch` — fetch news from sources
5. `news.fetch` with `depth: "deep"` — full fetch→enrich→index pipeline (replaces deprecated `intelligence.collect`)

### Default Output Format

**All bulk data tools return TOON by default** (token-efficient). Pass `format: "json"` for standard JSON.

### Tool Discovery (Progressive Loading)

Tools are organized into 7 domain groups. Load only the groups you need to conserve context.

| Group | Tools | Context Est. | When to Load |
|-------|-------|-------------|--------------|
| **Discovery** | `pools.*`, `sources.*`, `parsers.list` | ~5% | Initial setup, exploring available sources |
| **News** | `news.fetch`, `news.testSource`, `news.enrich` | ~3% | Fetching and enriching news articles |
| **Research** | `research.search`, `research.paper`, `research.download` | ~3% | Academic paper search and retrieval |
| **Web** | `web.search`, `web.scrape`, `web.crawl`, `web.map` | ~4% | Web search, scraping, crawling |
| **Insights** | `insights.findConnections`, `insights.trendingEntities`, `insights.indexArticles`, `insights.getStats`, `insights.clearIndex` | ~4% | Cross-article entity analysis |
| **Social** | `reddit.search`, `reddit.feed` | ~1% | Search Reddit posts and follow subreddit feeds |
| **Browser** | `lightpanda.*` (12 tools) | ~8% | JS-rendered browsing, form interaction |

**Recommended loading patterns:**

- **Quick scan**: Load Discovery + News (~8% context)
- **Full research**: Load Discovery + News + Research + Web + Insights (~19% context)
- **Social monitoring**: Load Discovery + News + Social (~9% context)
- **Browser automation**: Load Browser only (~8% context) after navigating

**Quick reference:**

| Group | Tools |
|-------|-------|
| **Discovery** | `pools.list`, `pools.upsert`, `pools.delete`, `sources.list`, `sources.upsert`, `sources.delete`, `sources.autodiscover`, `sources.enableGenericScraper`, `sources.countries`, `sources.cities`, `sources.domains`, `parsers.list` |
| **News** | `news.fetch`, `news.testSource`, `news.enrich` |
| **Research** | `research.search`, `research.paper`, `research.download` |
| **Web** | `web.search`, `web.scrape`, `web.crawl`, `web.map` |
| **Insights** | `insights.findConnections`, `insights.trendingEntities`, `insights.indexArticles`, `insights.getStats`, `insights.clearIndex` |
| **Social** | `reddit.search`, `reddit.feed` |
| **Browser** | `lightpanda.goto`, `lightpanda.markdown`, `lightpanda.links`, `lightpanda.evaluate`, `lightpanda.semantic_tree`, `lightpanda.structuredData`, `lightpanda.detectForms`, `lightpanda.click`, `lightpanda.fill`, `lightpanda.scroll`, `lightpanda.waitForSelector`, `lightpanda.interactiveElements` |

## Recommended Workflows

### 1. Quick Intelligence Collection

```
news.fetch(pools=["GLOBAL_TECH_CYBER"], limit=50, depth="deep")
→ Returns fetched count, enriched count, indexed count, stats

insights.trendingEntities(time_window_hours=24)
→ Returns entities with increasing mention frequency

insights.findConnections(min_domains=2)
→ Returns cross-domain connections. Entity is optional — omit for all connections.
```

### 2. Targeted News Monitoring

```
news.fetch(pools=["GLOBAL_BREAKING"], keywords=["earthquake"], limit=20)
→ Returns filtered news items

news.enrich(items=<from above>, extract=["topics","entities","sentiment"])
→ Returns enriched items with NLP data

insights.indexArticles(articles=<enriched items>)
→ Indexes for cross-article analysis
```

### 3. Deep Web Research

```
web.search(query="quantum computing breakthroughs", max_results=10)
→ Returns web search results

web.scrape(url=<interesting_url>, provider="lightpanda")
→ Returns structured markdown with metadata

web.crawl(url=<site>, max_depth=3, max_pages=50)
→ BFS crawl returning all pages with depth tracking
```

### 4. Academic Paper Research

```
research.search(query="transformer architecture", sources=["arxiv"], limit=10)
→ Returns papers with title, authors, abstract, year

research.paper(id="arxiv:2301.00001", include_citations=true)
→ Returns detailed paper info with citations

research.download(id="arxiv:2301.00001")
→ Downloads PDF to disk
```

### 5. Geographic Intelligence

```
sources.countries()
→ List countries with source counts

news.fetch(countries=["IN","US"], limit=20)
→ Fetch news from specific countries

news.fetch(cities=["Delhi","Mumbai"], limit=10)
→ Fetch news from specific cities
```

### 6. Browser Automation (Lightpanda MCP)

```
lightpanda.goto(url="https://example.com", wait_until="networkidle")
→ Navigate to page, render JavaScript

lightpanda.structuredData()
→ Extract JSON-LD, OpenGraph, microdata

lightpanda.detectForms()
→ Find forms on the page

lightpanda.fill(selector="input[name=email]", value="user@example.com")
→ Fill form field

lightpanda.click(selector="button[type=submit]", wait_for_navigation=true)
→ Click submit button

lightpanda.markdown()
→ Get page content as structured markdown

lightpanda.evaluate(expression="document.title")
→ Execute JavaScript and get result
```

## Tool Details

### Pool IDs

Available pools: `GLOBAL_BREAKING`, `GLOBAL_GEOECON`, `GLOBAL_LAW_REG`, `GLOBAL_TECH_CYBER`, `GLOBAL_ENV_HEALTH`, `GLOBAL_CULT_SOC`, `INDIA_NATIONAL_BASE`, `INDIA_WATCHDOG`, `INDIA_FACTCHECK_DATA`, `INDIA_BUSINESS_REG`, `INDIA_REGION`, `INDIA_CITIES`, `GLOBAL_COUNTRIES`, `GLOBAL_CITIES`, `GLOBAL_HEALTH`, `GLOBAL_ENVIRONMENT`, `GLOBAL_SCIENCE`, `GLOBAL_DEFENSE_SECURITY`

### Parser Keys

| Key | Use Case |
|-----|----------|
| `rss` | Standard RSS/Atom feeds (default) |
| `ofac` | US Treasury OFAC Recent Actions |
| `ussf_cfc` | US Space Force CFC News |
| `who_dons` | WHO Disease Outbreak News |
| `newslaundry` | Newslaundry JSON-in-script |
| `generic_html` | HTML scraping with CSS selectors |
| `semantic_scholar` | Semantic Scholar JSON API |

### Web Providers

| Provider | Tool | Requires |
|----------|------|----------|
| `default` | `web.scrape` | HTTP + html-to-markdown-rs |
| `lightpanda` | `web.scrape`, `web.crawl` | `lightpanda.enabled=true` in settings |
| `tavily` | `web.search` | `tavily.enabled=true` + API key |
| `firecrawl` | `web.search` | `firecrawl.enabled=true` + API key |

### web.crawl Options

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_depth` | 2 | BFS depth limit |
| `max_pages` | 20 | Max pages to crawl |
| `obey_robots` | true | Respect robots.txt |
| `dump_format` | "markdown" | Output format (markdown/html/semantic_tree) |
| `wait_until` | "networkidle" | When to capture (load/domcontentloaded/networkidle/done) |
| `wait_selector` | — | CSS selector to wait for |
| `strip_mode` | — | Strip content (js/css/ui/full) |
| `include_frames` | false | Include iframe content |

### web.scrape Options

| Parameter | Default | Description |
|-----------|---------|-------------|
| `provider` | "default" | "default" (HTTP) or "lightpanda" (JS rendering) |
| `wait_selector` | — | CSS selector to wait for (Lightpanda only) |
| `strip_mode` | — | Strip content (Lightpanda only) |
| `wait_until` | "networkidle" | When to capture (Lightpanda only) |
| `include_frames` | false | Include iframes (Lightpanda only) |

### Lightpanda MCP Browser Tools

These tools use a persistent browser session via `lightpanda mcp`. The page stays loaded between calls — navigate first, then interact.

| Tool | Parameters | Description |
|------|-----------|-------------|
| `lightpanda.goto` | `url`, `wait_until?` | Navigate to URL. Renders JavaScript. |
| `lightpanda.markdown` | `strip_mode?` | Get current page as markdown. |
| `lightpanda.links` | `selector?` | Extract links from current page. |
| `lightpanda.evaluate` | `expression` | Execute JavaScript. Returns result. |
| `lightpanda.semantic_tree` | `include_text?` | Get AI-friendly DOM tree. |
| `lightpanda.structuredData` | `jsonld?`, `opengraph?`, `microdata?` | Extract JSON-LD, OpenGraph, microdata. |
| `lightpanda.detectForms` | `selector?` | Find forms on current page. |
| `lightpanda.click` | `selector`, `wait_for_navigation?` | Click element by CSS selector. |
| `lightpanda.fill` | `selector`, `value` | Fill form field. |
| `lightpanda.scroll` | `direction?`, `pixels?` | Scroll page (up/down/left/right). |
| `lightpanda.waitForSelector` | `selector`, `timeout_ms?` | Wait for element to appear. |
| `lightpanda.interactiveElements` | `selector?` | Find clickable/fillable elements. |

### NLP Enrichment

`news.enrich` performs offline NLP (no external API calls):

| Feature | Method | Output |
|---------|--------|--------|
| Topics | Word frequency after stop-word removal | `Vec<String>` |
| Entities | Capitalization-based proper noun detection | `Vec<{name, type, mentions, confidence}>` |
| Sentiment | 38-word positive / 37-word negative lexicon | `{score, comparative, label}` |
| Summary | First sentence of content snippet | `String` |

### Insight Engine

After indexing articles via `insights.indexArticles` or `news.fetch` with `depth: "deep"`:

| Tool | Purpose |
|------|---------|
| `insights.findConnections(entity?, min_domains?, limit?)` | Find cross-domain connections. Entity optional — omit for all connections. |
| `insights.trendingEntities(time_window_hours, min_growth, min_current_mentions)` | Detect entity mention trends |
| `insights.getStats` | Engine statistics (total_articles, total_entities, total_domains) |
| `insights.clearIndex` | Clear all indexed articles |

The insight engine persists to SQLite at `~/.config/igs-mcp/insights.db`.

## Error Messages

IGS provides actionable error messages:

| Pattern | Example |
|---------|---------|
| Prerequisite | "Lightpanda is not enabled. Set lightpanda.enabled=true in settings.yml" |
| Configuration | "No web search provider available. Configure Tavily or Firecrawl in settings.yml." |
| Input validation | "Invalid URL 'not-a-url': relative URL without a base" |
| HTTP errors | "HTTP 404 for URL: https://example.com/missing" |
| Paper ID format | "Unknown paper ID format. Use arxiv:XXXX.XXXXX or semanticscholar:XXXX" |

## Configuration

Edit `~/.config/igs-mcp/settings.yml`:

```yaml
# Enable Lightpanda for JS-rendered crawling
lightpanda:
  enabled: true
  auto_update: true
  obey_robots: true
  timeout_ms: 30000

# Enable Tavily for web search
tavily:
  enabled: true
  apiKey: "tvly-YOUR_KEY"

# Restrict loaded tool groups (default: all)
tool_groups:
  - discovery
  - news
  - insights

# Change default output format
output:
  default_format: toon    # "toon" or "json"
```

## CLI

IGS also provides a CLI binary `igs` for direct command-line use:

```bash
# MCP server (for Claude Desktop / AI agents)
igs mcp

# CLI commands
igs status
igs news fetch --pools GLOBAL_TECH_CYBER --limit 10
igs web scrape --url https://example.com
igs research search --query "AI safety"
igs browser goto --url https://example.com
```

See `igs --help` for full command list.
