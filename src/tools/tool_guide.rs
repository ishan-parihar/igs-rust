use super::types::*;
use std::collections::HashMap;

/// Get tool guide with decision tree, categories, and drill-down chains.
pub async fn get_tool_guide() -> Result<ToolGuideOutput, String> {
    let mut decision_tree = HashMap::new();
    
    // News & Research
    decision_tree.insert("I need current news articles".to_string(), "news.fetch".to_string());
    decision_tree.insert("I need academic papers".to_string(), "research.search".to_string());
    decision_tree.insert("I need medical research papers".to_string(), "research.pubmed_search".to_string());
    decision_tree.insert("I need to search the web".to_string(), "web.search".to_string());
    decision_tree.insert("I need to scrape a website".to_string(), "web.scrape".to_string());
    decision_tree.insert("I need to monitor Reddit".to_string(), "reddit.search".to_string());
    decision_tree.insert("I need to download a paper".to_string(), "research.download".to_string());
    decision_tree.insert("I need to crawl a website".to_string(), "web.crawl".to_string());
    decision_tree.insert("I need to test a news source".to_string(), "news.testSource".to_string());
    
    // Weather & Climate
    decision_tree.insert("I need weather data".to_string(), "weather.forecast".to_string());
    decision_tree.insert("I need historical climate data".to_string(), "climate.noaa_observations".to_string());
    decision_tree.insert("I need weather stations".to_string(), "climate.noaa_stations".to_string());
    decision_tree.insert("I need current weather conditions".to_string(), "weather.current".to_string());
    decision_tree.insert("I need weather alerts".to_string(), "weather.alerts".to_string());
    
    // Finance & Business
    decision_tree.insert("I need market/financial data".to_string(), "finance.market".to_string());
    decision_tree.insert("I need cryptocurrency prices".to_string(), "finance.crypto".to_string());
    decision_tree.insert("I need stock market data".to_string(), "finance.market".to_string());
    decision_tree.insert("I need trending cryptocurrencies".to_string(), "finance.trending".to_string());

    // Security & Patents
    decision_tree.insert("I need vulnerability info".to_string(), "security.cve".to_string());
    decision_tree.insert("I need patent search".to_string(), "patents.search".to_string());
    decision_tree.insert("I need patent details".to_string(), "patents.details".to_string());
    decision_tree.insert("I need GitHub security advisories".to_string(), "security.advisories".to_string());
    
    // Government & Politics
    decision_tree.insert("I need congressional bills".to_string(), "govt.bills".to_string());
    decision_tree.insert("I need federal regulations".to_string(), "govt.regulations".to_string());
    decision_tree.insert("I need government regulations".to_string(), "govt.regulations".to_string());
    decision_tree.insert("I need campaign finance data".to_string(), "politics.fec_candidates".to_string());
    decision_tree.insert("I need political committees".to_string(), "politics.fec_committees".to_string());
    decision_tree.insert("I need FEC committee data".to_string(), "politics.fec_committees".to_string());

    // Health & Environment
    decision_tree.insert("I need health statistics".to_string(), "health.cdc_leading_causes".to_string());
    decision_tree.insert("I need global health data".to_string(), "health.who_gho".to_string());
    decision_tree.insert("I need fire hotspot data".to_string(), "satellite.firms_fires".to_string());
    decision_tree.insert("I need EPA facility data".to_string(), "env.epa_facilities".to_string());
    decision_tree.insert("I need EPA emissions data".to_string(), "env.epa_emissions".to_string());
    
    // Legal
    decision_tree.insert("I need case law search".to_string(), "legal.search_cases".to_string());
    decision_tree.insert("I need court case details".to_string(), "legal.case_details".to_string());
    
    // Insights & Discovery
    decision_tree.insert("I need to find cross-source connections".to_string(), "insights.findConnections".to_string());
    decision_tree.insert("I need to detect trending entities".to_string(), "insights.trendingEntities".to_string());
    decision_tree.insert("I need to index articles for analysis".to_string(), "insights.indexArticles".to_string());
    decision_tree.insert("I need insight engine statistics".to_string(), "insights.getStats".to_string());
    decision_tree.insert("I need to clear the insight index".to_string(), "insights.clearIndex".to_string());
    
    // Browser
    decision_tree.insert("I need to browse JS-rendered pages".to_string(), "lightpanda.goto".to_string());
    
    // Discovery & Source Management
    decision_tree.insert("I need to discover RSS feeds from a website".to_string(), "sources.autodiscover".to_string());
    decision_tree.insert("I need to discover RSS feeds".to_string(), "sources.autodiscover".to_string());
    decision_tree.insert("I need to list available sources".to_string(), "sources.list".to_string());
    decision_tree.insert("I need to list available pools".to_string(), "pools.list".to_string());
    decision_tree.insert("I need to create a news pool".to_string(), "pools.upsert".to_string());
    decision_tree.insert("I need to delete a news pool".to_string(), "pools.delete".to_string());
    decision_tree.insert("I need to add a news source".to_string(), "sources.upsert".to_string());
    decision_tree.insert("I need to remove a news source".to_string(), "sources.delete".to_string());
    decision_tree.insert("I need to scrape a website without RSS".to_string(), "sources.enableGenericScraper".to_string());
    decision_tree.insert("I need to list cities with sources".to_string(), "sources.cities".to_string());
    decision_tree.insert("I need to list domains with sources".to_string(), "sources.domains".to_string());
    
    // SOP & Workflows
    decision_tree.insert("I need to run multi-step workflows".to_string(), "sop.list".to_string());
    decision_tree.insert("I need to map a website structure".to_string(), "web.map".to_string());
    
    let mut categories = HashMap::new();
    
    // Discovery - all 13 tools
    categories.insert("Discovery".to_string(), vec![
        ToolGuideItem { name: "pools.list".to_string(), description: "List source pools".to_string() },
        ToolGuideItem { name: "pools.upsert".to_string(), description: "Create/update a pool".to_string() },
        ToolGuideItem { name: "pools.delete".to_string(), description: "Delete a pool".to_string() },
        ToolGuideItem { name: "sources.list".to_string(), description: "List news sources".to_string() },
        ToolGuideItem { name: "sources.upsert".to_string(), description: "Create/update a source".to_string() },
        ToolGuideItem { name: "sources.delete".to_string(), description: "Delete a source".to_string() },
        ToolGuideItem { name: "sources.autodiscover".to_string(), description: "Auto-discover RSS feeds".to_string() },
        ToolGuideItem { name: "sources.enableGenericScraper".to_string(), description: "Enable HTML scraping".to_string() },
        ToolGuideItem { name: "sources.countries".to_string(), description: "List countries with source counts".to_string() },
        ToolGuideItem { name: "sources.cities".to_string(), description: "List cities with source counts".to_string() },
        ToolGuideItem { name: "sources.domains".to_string(), description: "List domains with source counts".to_string() },
        ToolGuideItem { name: "parsers.list".to_string(), description: "List available parser types".to_string() },
        ToolGuideItem { name: "tool.guide".to_string(), description: "Decision tree for tool selection".to_string() },
    ]);
    
    // News - all 3 tools
    categories.insert("News".to_string(), vec![
        ToolGuideItem { name: "news.fetch".to_string(), description: "Fetch news from sources (depth=deep for full pipeline)".to_string() },
        ToolGuideItem { name: "news.testSource".to_string(), description: "Test a single source (returns up to 10 items)".to_string() },
        ToolGuideItem { name: "news.enrich".to_string(), description: "NLP enrichment (topics, entities, sentiment)".to_string() },
    ]);
    
    // Research - all 4 tools
    categories.insert("Research".to_string(), vec![
        ToolGuideItem { name: "research.search".to_string(), description: "Search arXiv + Semantic Scholar".to_string() },
        ToolGuideItem { name: "research.paper".to_string(), description: "Get paper details with citations".to_string() },
        ToolGuideItem { name: "research.download".to_string(), description: "Download paper PDF to disk".to_string() },
        ToolGuideItem { name: "research.pubmed_search".to_string(), description: "Search PubMed medical research".to_string() },
    ]);
    
    // Web - all 4 tools
    categories.insert("Web".to_string(), vec![
        ToolGuideItem { name: "web.search".to_string(), description: "Real-time web search (Tavily)".to_string() },
        ToolGuideItem { name: "web.scrape".to_string(), description: "Scrape URL to markdown".to_string() },
        ToolGuideItem { name: "web.crawl".to_string(), description: "BFS crawl website".to_string() },
        ToolGuideItem { name: "web.map".to_string(), description: "Discover URLs from sitemap".to_string() },
    ]);
    
    // Insights - all 5 tools
    categories.insert("Insights".to_string(), vec![
        ToolGuideItem { name: "insights.findConnections".to_string(), description: "Find cross-domain connections".to_string() },
        ToolGuideItem { name: "insights.trendingEntities".to_string(), description: "Detect trending entities".to_string() },
        ToolGuideItem { name: "insights.indexArticles".to_string(), description: "Index articles for analysis".to_string() },
        ToolGuideItem { name: "insights.getStats".to_string(), description: "Engine statistics".to_string() },
        ToolGuideItem { name: "insights.clearIndex".to_string(), description: "Clear all indexed articles".to_string() },
    ]);
    
    // Social - all 2 tools
    categories.insert("Social".to_string(), vec![
        ToolGuideItem { name: "reddit.search".to_string(), description: "Search Reddit posts".to_string() },
        ToolGuideItem { name: "reddit.feed".to_string(), description: "Follow subreddit feeds".to_string() },
    ]);
    
    // Weather - all 3 tools
    categories.insert("Weather".to_string(), vec![
        ToolGuideItem { name: "weather.forecast".to_string(), description: "Get weather forecast".to_string() },
        ToolGuideItem { name: "weather.current".to_string(), description: "Get current weather".to_string() },
        ToolGuideItem { name: "weather.alerts".to_string(), description: "Get weather alerts".to_string() },
    ]);
    
    // Finance - all 3 tools
    categories.insert("Finance".to_string(), vec![
        ToolGuideItem { name: "finance.market".to_string(), description: "Stock market quotes (Yahoo Finance)".to_string() },
        ToolGuideItem { name: "finance.crypto".to_string(), description: "Cryptocurrency prices (CoinGecko)".to_string() },
        ToolGuideItem { name: "finance.trending".to_string(), description: "Trending cryptocurrencies".to_string() },
    ]);
    
    // Security - all 2 tools
    categories.insert("Security".to_string(), vec![
        ToolGuideItem { name: "security.cve".to_string(), description: "Search CVE vulnerabilities (NVD)".to_string() },
        ToolGuideItem { name: "security.advisories".to_string(), description: "Search GitHub security advisories".to_string() },
    ]);
    
    // Patents - all 2 tools
    categories.insert("Patents".to_string(), vec![
        ToolGuideItem { name: "patents.search".to_string(), description: "Search USPTO patents".to_string() },
        ToolGuideItem { name: "patents.details".to_string(), description: "Get patent details".to_string() },
    ]);
    
    // Government - all 2 tools
    categories.insert("Government".to_string(), vec![
        ToolGuideItem { name: "govt.bills".to_string(), description: "Search congressional bills".to_string() },
        ToolGuideItem { name: "govt.regulations".to_string(), description: "Search federal regulations".to_string() },
    ]);
    
    // Politics - all 2 tools
    categories.insert("Politics".to_string(), vec![
        ToolGuideItem { name: "politics.fec_candidates".to_string(), description: "Search FEC candidates".to_string() },
        ToolGuideItem { name: "politics.fec_committees".to_string(), description: "Search FEC committees".to_string() },
    ]);
    
    // Health - all 2 tools
    categories.insert("Health".to_string(), vec![
        ToolGuideItem { name: "health.cdc_leading_causes".to_string(), description: "Leading causes of death (US)".to_string() },
        ToolGuideItem { name: "health.who_gho".to_string(), description: "Global health indicators (194 countries)".to_string() },
    ]);
    
    // Climate - all 2 tools
    categories.insert("Climate".to_string(), vec![
        ToolGuideItem { name: "climate.noaa_observations".to_string(), description: "Historical weather observations".to_string() },
        ToolGuideItem { name: "climate.noaa_stations".to_string(), description: "Find weather stations".to_string() },
    ]);
    
    // Legal - all 2 tools
    categories.insert("Legal".to_string(), vec![
        ToolGuideItem { name: "legal.search_cases".to_string(), description: "Search case law (CourtListener)".to_string() },
        ToolGuideItem { name: "legal.case_details".to_string(), description: "Get case details".to_string() },
    ]);
    
    // Environment - all 3 tools
    categories.insert("Environment".to_string(), vec![
        ToolGuideItem { name: "env.epa_facilities".to_string(), description: "EPA-regulated facilities".to_string() },
        ToolGuideItem { name: "env.epa_emissions".to_string(), description: "Toxic release inventory".to_string() },
        ToolGuideItem { name: "satellite.firms_fires".to_string(), description: "NASA FIRMS fire hotspots (satellite)".to_string() },
    ]);
    
    // SOP - all 2 tools
    categories.insert("SOP".to_string(), vec![
        ToolGuideItem { name: "sop.list".to_string(), description: "List available workflows".to_string() },
        ToolGuideItem { name: "sop.execute".to_string(), description: "Execute multi-step workflow".to_string() },
    ]);
    
    // Browser - all 12 tools
    categories.insert("Browser".to_string(), vec![
        ToolGuideItem { name: "lightpanda.goto".to_string(), description: "Navigate to URL (JS rendering)".to_string() },
        ToolGuideItem { name: "lightpanda.markdown".to_string(), description: "Get page as markdown".to_string() },
        ToolGuideItem { name: "lightpanda.links".to_string(), description: "Extract links".to_string() },
        ToolGuideItem { name: "lightpanda.evaluate".to_string(), description: "Execute JavaScript".to_string() },
        ToolGuideItem { name: "lightpanda.semantic_tree".to_string(), description: "AI-friendly DOM tree".to_string() },
        ToolGuideItem { name: "lightpanda.structuredData".to_string(), description: "Extract JSON-LD, OpenGraph".to_string() },
        ToolGuideItem { name: "lightpanda.detectForms".to_string(), description: "Find forms".to_string() },
        ToolGuideItem { name: "lightpanda.click".to_string(), description: "Click element".to_string() },
        ToolGuideItem { name: "lightpanda.fill".to_string(), description: "Fill form field".to_string() },
        ToolGuideItem { name: "lightpanda.scroll".to_string(), description: "Scroll page".to_string() },
        ToolGuideItem { name: "lightpanda.waitForSelector".to_string(), description: "Wait for element".to_string() },
        ToolGuideItem { name: "lightpanda.interactiveElements".to_string(), description: "Find clickable items".to_string() },
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
        DrillDownChain {
            name: "Policy Tracking".to_string(),
            description: "Search bills, regulations, and related news".to_string(),
            steps: vec!["govt.bills".to_string(), "govt.regulations".to_string(), "news.fetch".to_string()],
        },
        DrillDownChain {
            name: "Environmental Monitoring".to_string(),
            description: "Check fire hotspots, EPA facilities, and related news".to_string(),
            steps: vec!["satellite.firms_fires".to_string(), "env.epa_facilities".to_string(), "news.fetch".to_string()],
        },
    ];
    
    Ok(ToolGuideOutput {
        decision_tree,
        categories,
        drill_down_chains,
    })
}
