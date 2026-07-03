//! Tool registry with domain-based filtering for progressive discovery.
//! AI agents load 5-12 tools at a time instead of all 76.

/// Tool group definitions for progressive discovery.
/// Each group is a bounded context with 5-12 tools.
pub struct ToolGroup {
    pub name: &'static str,
    pub description: &'static str,
    pub tools: &'static [&'static str],
}

pub const TOOL_GROUPS: &[ToolGroup] = &[
    ToolGroup {
        name: "discovery",
        description: "Explore available pools, sources, parsers, and geographic coverage. Start here to understand what data is available.",
        tools: &[
            "pools.list", "pools.upsert", "pools.delete",
            "sources.list", "sources.upsert", "sources.delete",
            "sources.autodiscover", "sources.enable_generic_scraper",
            "sources.countries", "sources.cities", "sources.domains",
            "parsers.list",
        ],
    },
    ToolGroup {
        name: "news",
        description: "Fetch, test, and enrich news from RSS feeds and web crawling. Use news.fetch for general news gathering, news.fetch with depth=deep for full intelligence pipeline.",
        tools: &[
            "news.fetch", "news.test_source", "news.enrich",
        ],
    },
    ToolGroup {
        name: "research",
        description: "Search arXiv, Semantic Scholar, and PubMed for academic papers. Download PDFs for offline analysis.",
        tools: &[
            "research.search", "research.paper", "research.download", "research.pubmed_search",
        ],
    },
    ToolGroup {
        name: "web",
        description: "Search the web, scrape pages, crawl sites, and map website structures. Use browser tools for JavaScript-rendered pages.",
        tools: &[
            "web.search", "web.scrape", "web.crawl", "web.map",
        ],
    },
    ToolGroup {
        name: "insights",
        description: "Find cross-entity connections and trending topics across indexed articles. Requires prior news.fetch with depth=deep or insights.index_articles to populate the index.",
        tools: &[
            "insights.find_connections", "insights.trending_entities",
            "insights.get_stats", "insights.index_articles", "insights.clear_index",
        ],
    },
    ToolGroup {
        name: "social",
        description: "Search Reddit for posts and comments. Supports all subreddits with time filtering.",
        tools: &[
            "reddit.search", "reddit.feed",
        ],
    },
    ToolGroup {
        name: "twitter",
        description: "Search and read tweets via Twitter/X. Uses cookie-based authentication. Requires twitter.enabled=true and twitter.cookie in settings.yml.",
        tools: &[
            "twitter.search", "twitter.read",
        ],
    },
    ToolGroup {
        name: "weather",
        description: "Get weather forecasts, current conditions, and severe weather alerts for any location. Uses OpenWeatherMap free tier API.",
        tools: &[
            "weather.forecast", "weather.current", "weather.alerts",
        ],
    },
    ToolGroup {
        name: "finance",
        description: "Get stock market quotes, cryptocurrency prices, and trending coins. Uses Yahoo Finance for stocks and CoinGecko for crypto (both free, no API key required).",
        tools: &[
            "finance.market", "finance.crypto", "finance.trending",
        ],
    },
    ToolGroup {
        name: "security",
        description: "Search CVE vulnerabilities and GitHub Security Advisories. Use for threat intelligence, vulnerability monitoring, and dependency security.",
        tools: &[
            "security.cve", "security.advisories",
        ],
    },
    ToolGroup {
        name: "patents",
        description: "Search USPTO patents and retrieve patent details. Use for intellectual property research, prior art searches, and technology landscape analysis.",
        tools: &[
            "patents.search", "patents.details",
        ],
    },
    ToolGroup {
        name: "government",
        description: "Search US Congressional bills and Federal Register regulations. Use for legislative tracking, regulatory monitoring, and policy intelligence.",
        tools: &[
            "govt.bills", "govt.regulations",
        ],
    },
    ToolGroup {
        name: "legal",
        description: "Search US court cases via CourtListener API. Find case law, dockets, and legal opinions across federal and state courts.",
        tools: &[
            "legal.search_cases", "legal.case_details",
        ],
    },
    ToolGroup {
        name: "environment",
        description: "EPA environmental facility and emissions data via Envirofacts API, plus satellite-based fire detection via NASA FIRMS. Query regulated facilities, TRI emissions, fire hotspots, and environmental compliance data.",
        tools: &[
            "env.epa_facilities", "env.epa_emissions", "satellite.firms_fires",
        ],
    },
    ToolGroup {
        name: "climate",
        description: "NOAA Climate Data Online for historical weather observations and station data. Query daily/monthly/yearly climate records by location.",
        tools: &[
            "climate.noaa_observations", "climate.noaa_stations",
        ],
    },
    ToolGroup {
        name: "health",
        description: "WHO Global Health Observatory data and CDC health statistics. Query global health indicators, disease data, and vital statistics.",
        tools: &[
            "health.cdc_leading_causes", "health.who_gho",
        ],
    },
    ToolGroup {
        name: "politics",
        description: "Campaign finance and political data. Search FEC candidates and committees.",
        tools: &[
            "politics.fec_candidates", "politics.fec_committees",
        ],
    },
    ToolGroup {
        name: "browser",
        description: "Stateless per-call browser fetch via Obscura CLI. Each tool invocation re-renders the page (no persistent session). Use browser.goto first to set the current URL, then browser.markdown/browser.links/etc. will fetch that URL.",
        tools: &[
            "browser.goto", "browser.markdown", "browser.links",
            "browser.evaluate",
            "browser.click", "browser.fill", "browser.scroll",
            "browser.wait_for_selector",
        ],
    },
    ToolGroup {
        name: "sop",
        description: "Composable multi-step intelligence workflows. List built-in chains or execute them with parameterized queries.",
        tools: &[
            "sop.list", "sop.execute",
        ],
    },
    ToolGroup {
        name: "monitor",
        description: "Real-time monitoring & alerting. Create monitors that poll news sources for keyword matches and trigger webhook/file alerts.",
        tools: &[
            "monitor.create", "monitor.list",
            "monitor.delete", "monitor.pause", "monitor.resume",
        ],
    },
    ToolGroup {
        name: "intelligence",
        description: "Advanced intelligence capabilities: TextRank summarization, entity resolution with alias detection, GDELT global events database.",
        tools: &[
            "news.summarize", "entities.resolve", "search.gdelt",
        ],
    },
    ToolGroup {
        name: "advanced",
        description: "Advanced analysis: temporal anomaly detection, geospatial location extraction, language detection, source quality scoring, report generation, semantic search.",
        tools: &[
            "advanced.temporal_analysis", "advanced.extract_locations",
            "advanced.detect_language", "advanced.source_quality",
            "advanced.generate_report", "search.semantic",
        ],
    },
    ToolGroup {
        name: "youtube",
        description: "Search YouTube videos, extract metadata and subtitles. Uses yt-dlp for data extraction.",
        tools: &[
            "youtube.search", "youtube.metadata", "youtube.subtitles",
        ],
    },
];

/// Get tools available for a specific group name.
pub fn get_group_tools(group_name: &str) -> Option<&'static [&'static str]> {
    TOOL_GROUPS
        .iter()
        .find(|g| g.name == group_name)
        .map(|g| g.tools)
}

/// Get all available group names.
pub fn list_groups() -> Vec<(&'static str, &'static str)> {
    TOOL_GROUPS
        .iter()
        .map(|g| (g.name, g.description))
        .collect()
}

/// Get total count of tools across all groups.
pub fn total_tool_count() -> usize {
    TOOL_GROUPS.iter().map(|g| g.tools.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_duplicate_tools_across_groups() {
        let mut all_tools = Vec::new();
        for group in TOOL_GROUPS {
            for tool in group.tools {
                assert!(
                    !all_tools.contains(tool),
                    "Tool '{}' appears in multiple groups",
                    tool
                );
                all_tools.push(tool);
            }
        }
    }

    #[test]
    fn test_all_groups_have_tools() {
        for group in TOOL_GROUPS {
            assert!(
                !group.tools.is_empty(),
                "Group '{}' has no tools",
                group.name
            );
        }
    }

    #[test]
    fn test_registry_tools_match_expected() {
        let expected = vec![
            "pools.list",
            "pools.upsert",
            "pools.delete",
            "sources.list",
            "sources.upsert",
            "sources.delete",
            "sources.autodiscover",
            "sources.enable_generic_scraper",
            "sources.countries",
            "sources.cities",
            "sources.domains",
            "parsers.list",
            "news.fetch",
            "news.test_source",
            "news.enrich",
            "reddit.search",
            "reddit.feed",
            "research.search",
            "research.paper",
            "research.download",
            "research.pubmed_search",
            "web.search",
            "web.scrape",
            "web.crawl",
            "web.map",
            "insights.find_connections",
            "insights.trending_entities",
            "insights.index_articles",
            "insights.get_stats",
            "insights.clear_index",
            "weather.forecast",
            "weather.current",
            "weather.alerts",
            "finance.market",
            "finance.crypto",
            "finance.trending",
            "security.cve",
            "security.advisories",
            "patents.search",
            "patents.details",
            "govt.bills",
            "govt.regulations",
            "env.epa_facilities",
            "env.epa_emissions",
            "satellite.firms_fires",
            "health.cdc_leading_causes",
            "health.who_gho",
            "politics.fec_candidates",
            "politics.fec_committees",
            "climate.noaa_observations",
            "climate.noaa_stations",
            "legal.search_cases",
            "legal.case_details",
            "browser.goto",
            "browser.markdown",
            "browser.links",
            "browser.evaluate",
            "browser.click",
            "browser.fill",
            "browser.scroll",
            "browser.wait_for_selector",
            "sop.list",
            "sop.execute",
            "monitor.create",
            "monitor.list",
            "monitor.delete",
            "monitor.pause",
            "monitor.resume",
            "news.summarize",
            "entities.resolve",
            "search.gdelt",
            "advanced.temporal_analysis",
            "advanced.extract_locations",
            "advanced.detect_language",
            "advanced.source_quality",
            "advanced.generate_report",
            "search.semantic",
            "youtube.search",
            "youtube.metadata",
            "youtube.subtitles",
            "twitter.search",
            "twitter.read",
        ];

        let registry_tools: Vec<&str> = TOOL_GROUPS
            .iter()
            .flat_map(|g| g.tools.iter())
            .copied()
            .collect();

        for tool in &expected {
            assert!(
                registry_tools.contains(tool),
                "Tool '{}' not in registry",
                tool
            );
        }
        for tool in &registry_tools {
            assert!(
                expected.contains(tool),
                "Tool '{}' in registry but not in expected list",
                tool
            );
        }
    }
}
