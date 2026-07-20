---
name: igs
description: >
  Intelligence Gathering System — 64 tools, 411 sources, 47 countries.
  Fetch news, search research, browse web, analyze insights, and monitor
  geopolitics, tech, defense, health, and more.
---

# IGS Skill

Intelligence Gathering System — 64 tools across 20 domains for comprehensive intelligence collection.

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

## Key Tools (64 total)

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

## Intelligence Pools (14)

GLOBAL_TECH_CYBER, INDIA_FOCUS, DEFENSE_SECURITY, HEALTH_MEDICAL, ENERGY_CLIMATE, FINANCE_CRYPTO, LEGAL_REGULATORY, POLITICS_GOVERNMENT, SOCIAL_MEDIA, SCIENCE_RESEARCH, ENVIRONMENT, PATENTS_IP, EMERGING_TECH, GEOPOLITICS

## Configuration

```bash
# Config directory
~/.config/igs-mcp/
├── settings.yml      # Main config
├── pools.yml         # Pool definitions
├── sources.yml       # 411 source definitions
└── countries.yml     # 47 country metadata
```

