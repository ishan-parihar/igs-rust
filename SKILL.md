---
name: igs
description: >
  Intelligence Gathering System — 91 tools, 418 sources, 45 countries.
  Fetch news, search research, browse web, analyze insights, and monitor
  geopolitics, tech, defense, health, and more.
---

# IGS Skill

Intelligence Gathering System — 91 tools across 25 domains for comprehensive intelligence collection.

<!-- Static skill — regenerate from CLI: igs --help -->
<!-- Install: npx skills add ishan-parihar/igs-rust --skill igs -->
<!-- CI check: diff <(igs --help) SKILL.md && exit 1 -->

## Quick Start

```bash
# Start MCP server for AI agents
igs mcp

# Or use CLI directly
igs status
igs news fetch --pools GLOBAL_TECH_CYBER --limit 10
```

## MCP Configuration

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

## Key Tools (91 total)

| Category | Tools | Description |
|----------|-------|-------------|
| Discovery | 13 | Pool/source CRUD, autodiscover, countries |
| News | 3 | Fetch, test, enrich |
| Research | 4 | arXiv, Semantic Scholar, PubMed |
| Web | 4 | Search, scrape, crawl, map |
| Insights | 5 | Connections, trending, indexing |
| Social | 2 | Reddit search/feed |
| Weather | 3 | Forecast, current, alerts |
| Finance | 3 | Stocks, crypto, trending |
| Security | 2 | CVE, advisories |
| Patents | 2 | Search, details |
| Government | 2 | Bills, regulations |
| Politics | 2 | FEC candidates/committees |
| Health | 2 | CDC, WHO |
| Environment | 3 | EPA, NASA FIRMS |
| Climate | 2 | NOAA observations/stations |
| Legal | 2 | Case law search/details |
| SOP | 2 | List/execute workflows |
| Browser | 8 | Headless browsing (Lightpanda) |

## Intelligence Pools (18)

GLOBAL_BREAKING, GLOBAL_GEOECON, GLOBAL_LAW_REG, GLOBAL_TECH_CYBER, GLOBAL_ENV_HEALTH, GLOBAL_CULT_SOC, INDIA_NATIONAL_BASE, INDIA_WATCHDOG, INDIA_FACTCHECK_DATA, INDIA_BUSINESS_REG, INDIA_REGION, INDIA_CITIES, GLOBAL_COUNTRIES, GLOBAL_CITIES, GLOBAL_HEALTH, GLOBAL_ENVIRONMENT, GLOBAL_SCIENCE, GLOBAL_DEFENSE_SECURITY

## Configuration

```bash
# Config directory
~/.config/igs-mcp/
├── settings.yml      # Main config
├── pools.yml         # Pool definitions
├── sources.yml       # 418 source definitions
└── countries.yml     # 45 country metadata
```

