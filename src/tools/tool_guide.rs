use super::types::*;
use std::collections::HashMap;

/// Get tool guide with decision tree, categories, and drill-down chains.
pub async fn get_tool_guide() -> Result<ToolGuideOutput, String> {
    let mut decision_tree = HashMap::new();
    decision_tree.insert("I need current news articles".to_string(), "news.fetch".to_string());
    decision_tree.insert("I need to scrape a website".to_string(), "web.scrape".to_string());
    decision_tree.insert("I need to search the web".to_string(), "web.search".to_string());
    decision_tree.insert("I need academic papers".to_string(), "research.search".to_string());
    decision_tree.insert("I need to monitor Reddit".to_string(), "reddit.search".to_string());
    decision_tree.insert("I need weather data".to_string(), "weather.forecast".to_string());
    decision_tree.insert("I need market/financial data".to_string(), "finance.market".to_string());
    decision_tree.insert("I need vulnerability info".to_string(), "security.cve".to_string());
    decision_tree.insert("I need to find cross-source connections".to_string(), "insights.findConnections".to_string());
    decision_tree.insert("I need to detect trending entities".to_string(), "insights.trendingEntities".to_string());
    decision_tree.insert("I need to browse JS-rendered pages".to_string(), "lightpanda.goto".to_string());
    decision_tree.insert("I need to discover RSS feeds".to_string(), "sources.autodiscover".to_string());
    decision_tree.insert("I need to list available sources".to_string(), "sources.list".to_string());
    decision_tree.insert("I need to list available pools".to_string(), "pools.list".to_string());
    
    let mut categories = HashMap::new();
    categories.insert("Discovery".to_string(), vec![
        ToolGuideItem { name: "pools.list".to_string(), description: "List source pools".to_string() },
        ToolGuideItem { name: "sources.list".to_string(), description: "List news sources".to_string() },
        ToolGuideItem { name: "sources.countries".to_string(), description: "List countries".to_string() },
        ToolGuideItem { name: "parsers.list".to_string(), description: "List parser types".to_string() },
    ]);
    categories.insert("News".to_string(), vec![
        ToolGuideItem { name: "news.fetch".to_string(), description: "Fetch news from sources".to_string() },
        ToolGuideItem { name: "news.enrich".to_string(), description: "NLP enrichment (topics, entities, sentiment)".to_string() },
    ]);
    categories.insert("Research".to_string(), vec![
        ToolGuideItem { name: "research.search".to_string(), description: "Search arXiv + Semantic Scholar".to_string() },
        ToolGuideItem { name: "research.paper".to_string(), description: "Get paper details with citations".to_string() },
    ]);
    categories.insert("Web".to_string(), vec![
        ToolGuideItem { name: "web.search".to_string(), description: "Real-time web search".to_string() },
        ToolGuideItem { name: "web.scrape".to_string(), description: "Scrape URL to markdown".to_string() },
        ToolGuideItem { name: "web.crawl".to_string(), description: "BFS crawl website".to_string() },
    ]);
    categories.insert("Insights".to_string(), vec![
        ToolGuideItem { name: "insights.findConnections".to_string(), description: "Find cross-domain connections".to_string() },
        ToolGuideItem { name: "insights.trendingEntities".to_string(), description: "Detect trending entities".to_string() },
    ]);
    categories.insert("Social".to_string(), vec![
        ToolGuideItem { name: "reddit.search".to_string(), description: "Search Reddit posts".to_string() },
        ToolGuideItem { name: "reddit.feed".to_string(), description: "Follow subreddit feeds".to_string() },
    ]);
    categories.insert("Security".to_string(), vec![
        ToolGuideItem { name: "security.cve".to_string(), description: "Search CVE vulnerabilities".to_string() },
        ToolGuideItem { name: "security.advisories".to_string(), description: "Search GitHub advisories".to_string() },
    ]);
    categories.insert("Weather".to_string(), vec![
        ToolGuideItem { name: "weather.forecast".to_string(), description: "Get weather forecast".to_string() },
        ToolGuideItem { name: "weather.current".to_string(), description: "Get current weather".to_string() },
        ToolGuideItem { name: "weather.alerts".to_string(), description: "Get weather alerts".to_string() },
    ]);
    
    let drill_down_chains = vec![
        DrillDownChain {
            name: "Deep Research Pipeline".to_string(),
            description: "Search web, scrape top results, enrich with NLP, index for insights".to_string(),
            steps: vec!["web.search".to_string(), "web.scrape".to_string(), "news.enrich".to_string(), "insights.indexArticles".to_string()],
        },
        DrillDownChain {
            name: "Threat Monitoring".to_string(),
            description: "Search CVEs, fetch related news, find connections".to_string(),
            steps: vec!["security.cve".to_string(), "news.fetch".to_string(), "insights.findConnections".to_string()],
        },
        DrillDownChain {
            name: "Competitive Intelligence".to_string(),
            description: "Search news, research papers, find trending entities".to_string(),
            steps: vec!["news.fetch".to_string(), "research.search".to_string(), "insights.trendingEntities".to_string()],
        },
    ];
    
    Ok(ToolGuideOutput {
        decision_tree,
        categories,
        drill_down_chains,
    })
}
