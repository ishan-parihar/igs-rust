//! Tool registry with domain-based filtering for progressive discovery.
//! AI agents load 5-12 tools at a time instead of all 41.

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
            "sources.autodiscover", "sources.enableGenericScraper",
            "sources.countries", "sources.cities", "sources.domains",
            "parsers.list",
        ],
    },
    ToolGroup {
        name: "news",
        description: "Fetch, test, and enrich news from RSS feeds and web crawling. Use news.fetch for general news gathering, news.fetch with depth=deep for full intelligence pipeline.",
        tools: &[
            "news.fetch", "news.testSource", "news.enrich",
        ],
    },
    ToolGroup {
        name: "research",
        description: "Search arXiv and Semantic Scholar for academic papers. Download PDFs for offline analysis.",
        tools: &[
            "research.search", "research.paper", "research.download",
        ],
    },
    ToolGroup {
        name: "web",
        description: "Search the web, scrape pages, crawl sites, and map website structures. Use lightpanda tools for JavaScript-rendered pages.",
        tools: &[
            "web.search", "web.scrape", "web.crawl", "web.map",
        ],
    },
    ToolGroup {
        name: "insights",
        description: "Find cross-entity connections and trending topics across indexed articles. Requires prior news.fetch with depth=deep or insights.indexArticles to populate the index.",
        tools: &[
            "insights.findConnections", "insights.trendingEntities",
            "insights.getStats", "insights.indexArticles", "insights.clearIndex",
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
        name: "browser",
        description: "Persistent browser session for JavaScript-rendered pages. Navigate with lightpanda.goto first, then interact with other tools.",
        tools: &[
            "lightpanda.goto", "lightpanda.markdown", "lightpanda.links",
            "lightpanda.evaluate", "lightpanda.semantic_tree",
            "lightpanda.structuredData", "lightpanda.detectForms",
            "lightpanda.click", "lightpanda.fill", "lightpanda.scroll",
            "lightpanda.waitForSelector", "lightpanda.interactiveElements",
        ],
    },
];

/// Get tools available for a specific group name.
pub fn get_group_tools(group_name: &str) -> Option<&'static [&'static str]> {
    TOOL_GROUPS.iter()
        .find(|g| g.name == group_name)
        .map(|g| g.tools)
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
        None => tool_names.to_vec(), // Unknown group = return all
    }
}

/// Get total count of tools across all groups.
pub fn total_tool_count() -> usize {
    TOOL_GROUPS.iter().map(|g| g.tools.len()).sum()
}
