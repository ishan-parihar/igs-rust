use clap::{Parser, Subcommand};
use igs_rust_mcp::server::IgsMcpServer;
use igs_rust_mcp::tools::types::*;
use igs_rust_mcp::tools::types_base::{
    DepthOptions, DiscoveryFilters, KeywordFilter, OutputOptions,
};
use igs_rust_mcp::tools::types::LimitInput;
use igs_rust_mcp::tools::{
    climate, env, finance, govt, health, helpers, insights, legal, news,
    parsers as parsers_tools, patents, politics, pools, reddit, registry, research,
    satellite, security, sop, sources, twitter, weather, web, youtube,
};
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "igs", version, about = "IGS — Intelligence Gathering System")]
struct Cli {
    /// Output format: "toon" (default) or "json"
    #[arg(long, default_value = "toon", global = true)]
    format: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start MCP server on stdio (for Claude Desktop, Cursor, AI agents)
    Mcp,
    /// Pool management
    Pools {
        #[command(subcommand)]
        action: PoolAction,
    },
    /// Source management
    Sources {
        #[command(subcommand)]
        action: SourceAction,
    },
    /// News fetching and enrichment
    News {
        #[command(subcommand)]
        action: NewsAction,
    },
    /// Reddit search
    Reddit {
        #[command(subcommand)]
        action: RedditAction,
    },
    /// Academic paper research
    Research {
        #[command(subcommand)]
        action: ResearchAction,
    },
    /// Web search, scrape, crawl, map
    Web {
        #[command(subcommand)]
        action: WebAction,
    },
    /// Twitter/X search and read
    Twitter {
        #[command(subcommand)]
        action: TwitterAction,
    },
    /// YouTube search, metadata, and subtitles
    Youtube {
        #[command(subcommand)]
        action: YoutubeAction,
    },
    /// Browser automation (persistent session)
    Browser {
        #[command(subcommand)]
        action: BrowserAction,
    },
    /// Weather forecasts, current conditions, and alerts
    Weather {
        #[command(subcommand)]
        action: WeatherAction,
    },
    /// Stock market, cryptocurrency, and trending coins
    Finance {
        #[command(subcommand)]
        action: FinanceAction,
    },
    /// CVE vulnerabilities and security advisories
    Security {
        #[command(subcommand)]
        action: SecurityAction,
    },
    /// US Congressional bills and Federal Register regulations
    Govt {
        #[command(subcommand)]
        action: GovtAction,
    },
    /// FEC campaign finance candidates and committees
    Politics {
        #[command(subcommand)]
        action: PoliticsAction,
    },
    /// USPTO patent search and details
    Patents {
        #[command(subcommand)]
        action: PatentsAction,
    },
    /// NASA FIRMS satellite fire detection
    Satellite {
        #[command(subcommand)]
        action: SatelliteAction,
    },
    /// EPA facilities and emissions data
    Env {
        #[command(subcommand)]
        action: EnvAction,
    },
    /// US court cases via CourtListener
    Legal {
        #[command(subcommand)]
        action: LegalAction,
    },
    /// CDC health statistics and WHO global health data
    Health {
        #[command(subcommand)]
        action: HealthAction,
    },
    /// NOAA climate observations and stations
    Climate {
        #[command(subcommand)]
        action: ClimateAction,
    },
    /// Cross-article entity analysis and trending detection
    Insights {
        #[command(subcommand)]
        action: InsightsAction,
    },
    /// Multi-step intelligence workflow chains
    Sop {
        #[command(subcommand)]
        action: SopAction,
    },
    /// Real-time monitoring & alerting
    Monitor {
        #[command(subcommand)]
        action: MonitorAction,
    },
    /// Advanced intelligence: TextRank summarization, entity resolution, GDELT
    Intelligence {
        #[command(subcommand)]
        action: IntelligenceAction,
    },
    /// Advanced analysis: temporal, geo, language, source quality, reports, semantic search
    Advanced {
        #[command(subcommand)]
        action: AdvancedAction,
    },
    /// Plugin system: webhook enrichment, script hooks, export
    Plugins {
        #[command(subcommand)]
        action: PluginsAction,
    },
    /// OSINT data sources: OpenAlex, Shodan, HIBP, ACLED
    Osint {
        #[command(subcommand)]
        action: OsintAction,
    },
    /// List available parsers
    Parsers,
    /// Show IGS settings and status
    Status,
    /// List tool groups for progressive discovery
    ToolGroups {
        /// Filter to show tools in a specific group
        #[arg(long)]
        group: Option<String>,
    },
}

#[derive(Subcommand)]
enum PoolAction {
    /// List all pools
    List,
    /// Create or update a pool
    Upsert {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a pool
    Delete {
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
enum SourceAction {
    /// List sources
    List {
        #[arg(long)]
        pool: Option<String>,
        #[arg(long)]
        active_only: bool,
    },
    /// Create or update a source
    Upsert {
        #[arg(long)]
        name: String,
        #[arg(long)]
        source_type: String,
        #[arg(long)]
        url: String,
        #[arg(long, value_delimiter = ',')]
        pools: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        countries: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        cities: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        domains: Option<Vec<String>>,
        #[arg(long)]
        parser: Option<String>,
        #[arg(long)]
        active: Option<bool>,
    },
    /// Delete a source by ID
    Delete {
        #[arg(long)]
        id: String,
    },
    /// Enable generic HTML scraping for a source
    EnableScraper {
        #[arg(long)]
        id: String,
        #[arg(long)]
        list_url: Option<String>,
        #[arg(long)]
        item_selector: Option<String>,
        #[arg(long)]
        title_selector: Option<String>,
        #[arg(long)]
        link_selector: Option<String>,
        #[arg(long)]
        date_selector: Option<String>,
    },
    /// Auto-discover feeds from a URL
    Discover {
        #[arg(long)]
        url: String,
        #[arg(long)]
        pool: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// List countries with source counts
    Countries,
    /// List cities with source counts
    Cities,
    /// List domains with source counts
    Domains,
}

#[derive(Subcommand)]
enum NewsAction {
    /// Fetch news from configured sources
    Fetch {
        #[arg(long, value_delimiter = ',')]
        pools: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        sources: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        countries: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        cities: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        domains: Option<Vec<String>>,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
        #[arg(long, value_delimiter = ',')]
        keywords: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        exclude_keywords: Option<Vec<String>>,
        #[arg(long)]
        match_all: bool,
        #[arg(long, default_value = "50")]
        limit: i32,
        #[arg(long, default_value = "prefer")]
        cache_mode: String,
        /// Fetch depth: "quick" (10 sources, 20 results), "deep" (200 sources, 500 results), or omit for default (100 sources, 100 results)
        #[arg(long)]
        depth: Option<String>,
        /// Skip NLP enrichment (depth=deep only)
        #[arg(long)]
        skip_enrich: bool,
        /// Skip insight indexing (depth=deep only)
        #[arg(long)]
        skip_index: bool,
    },
    /// Test a single source
    Test {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "bypass")]
        cache_mode: String,
    },
    /// Enrich news items with NLP
    Enrich {
        /// JSON file with items, or - for stdin
        #[arg(long)]
        input: Option<String>,
        /// What to extract: topics, entities, sentiment, summary, diversity (comma-separated)
        #[arg(long, value_delimiter = ',')]
        extract: Option<Vec<String>>,
    },
}

#[derive(Subcommand)]
enum RedditAction {
    /// Search Reddit posts
    Search {
        #[arg(long)]
        query: String,
        #[arg(long, value_delimiter = ',')]
        subreddits: Option<Vec<String>>,
        #[arg(long, default_value = "relevance")]
        sort: String,
        #[arg(long, default_value = "all")]
        time: String,
        #[arg(long, default_value = "25")]
        limit: i32,
    },
    /// Fetch latest posts via RSS feeds (reliable, no API key needed)
    Feed {
        #[arg(long, value_delimiter = ',')]
        subreddits: Vec<String>,
        #[arg(long, default_value = "25")]
        limit: i32,
    },
}

#[derive(Subcommand)]
enum ResearchAction {
    /// Search academic papers
    Search {
        #[arg(long)]
        query: String,
        #[arg(long, value_delimiter = ',', default_value = "arxiv,semanticscholar")]
        sources: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        categories: Option<Vec<String>>,
        #[arg(long)]
        year_from: Option<i32>,
        #[arg(long)]
        year_to: Option<i32>,
        #[arg(long, default_value = "25")]
        limit: i32,
    },
    /// Get paper details by ID
    Paper {
        #[arg(long)]
        id: String,
        /// Include list of citing papers
        #[arg(long)]
        include_citations: bool,
        /// Include list of referenced papers
        #[arg(long)]
        include_references: bool,
    },
    /// Download a paper PDF
    Download {
        #[arg(long)]
        id: String,
        #[arg(long)]
        output: Option<String>,
        /// Convert PDF to markdown sidecar file
        #[arg(long)]
        convert_to_markdown: bool,
    },
    /// Search PubMed for medical research papers
    PubMedSearch {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "20")]
        limit: i32,
    },
}

#[derive(Subcommand)]
enum WebAction {
    /// Web search via Tavily/Firecrawl
    Search {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "10")]
        max_results: i32,
        #[arg(long)]
        topic: Option<String>,
        #[arg(long, value_delimiter = ',')]
        include_domains: Option<Vec<String>>,
        #[arg(long, value_delimiter = ',')]
        exclude_domains: Option<Vec<String>>,
        /// Search provider: auto, tavily, or firecrawl
        #[arg(long)]
        provider: Option<String>,
        /// Days back (news topic only)
        #[arg(long)]
        days: Option<i32>,
        /// Include LLM-generated answer in results
        #[arg(long)]
        include_answer: bool,
    },
    /// Scrape a URL to structured markdown
    Scrape {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "default")]
        provider: String,
        #[arg(long)]
        wait_selector: Option<String>,
        #[arg(long)]
        strip_mode: Option<String>,
        #[arg(long)]
        wait_until: Option<String>,
        #[arg(long)]
        include_frames: bool,
    },
    /// Crawl a website using Obscura
    Crawl {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "2")]
        max_depth: i32,
        #[arg(long, default_value = "20")]
        max_pages: i32,
        #[arg(long)]
        obey_robots: bool,
        #[arg(long, default_value = "markdown")]
        dump_format: String,
        #[arg(long)]
        wait_selector: Option<String>,
    },
    /// Discover URLs via sitemap.xml
    Map {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "100")]
        limit: i32,
        #[arg(long)]
        search: Option<String>,
    },
}

#[derive(Subcommand)]
enum TwitterAction {
    Search {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "10")]
        limit: i32,
        #[arg(long)]
        mode: Option<String>,
    },
    Read {
        #[arg(long)]
        url: String,
    },
}

#[derive(Subcommand)]
enum YoutubeAction {
    Search {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "10")]
        limit: i32,
    },
    Metadata {
        #[arg(long)]
        url: String,
    },
    Subtitles {
        #[arg(long)]
        url: String,
        #[arg(long)]
        lang: Option<String>,
    },
}

#[derive(Subcommand)]
enum BrowserAction {
    /// Navigate to a URL
    Goto {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "networkidle")]
        wait_until: String,
    },
    /// Get current page as markdown
    Markdown {
        #[arg(long)]
        strip_mode: Option<String>,
    },
    /// Extract links from current page
    Links {
        #[arg(long)]
        selector: Option<String>,
    },
    /// Execute JavaScript
    Evaluate {
        #[arg(long)]
        expression: String,
    },
    /// Click an element
    Click {
        #[arg(long)]
        selector: String,
        #[arg(long)]
        wait_for_navigation: bool,
    },
    /// Fill a form field
    Fill {
        #[arg(long)]
        selector: String,
        #[arg(long)]
        value: String,
    },
    /// Scroll the page
    Scroll {
        #[arg(long, default_value = "down")]
        direction: String,
        #[arg(long, default_value = "500")]
        pixels: i32,
    },
    /// Wait for element to appear
    WaitForSelector {
        #[arg(long)]
        selector: String,
        #[arg(long, default_value = "5000")]
        timeout_ms: u64,
    },
}

// ─── New Action Enums for MCP/CLI Parity ───────────────────────

#[derive(Subcommand)]
enum WeatherAction {
    /// Get weather forecast for a location
    Forecast {
        #[arg(long)]
        location: String,
        #[arg(long)]
        days: Option<u32>,
    },
    /// Get current weather for a location
    Current {
        #[arg(long)]
        location: String,
    },
    /// Get weather alerts for a lat/lon
    Alerts {
        #[arg(long)]
        lat: f64,
        #[arg(long)]
        lon: f64,
    },
}

#[derive(Subcommand)]
enum FinanceAction {
    /// Get stock market quotes
    Market {
        #[arg(long, value_delimiter = ',')]
        symbols: Vec<String>,
    },
    /// Get cryptocurrency prices
    Crypto {
        #[arg(long, value_delimiter = ',')]
        symbols: Vec<String>,
    },
    /// Get trending cryptocurrencies
    Trending,
}

#[derive(Subcommand)]
enum SecurityAction {
    /// Search CVE vulnerabilities
    Cve {
        #[arg(long)]
        query: String,
        #[arg(long)]
        days_back: Option<i32>,
    },
    /// Search GitHub Security Advisories
    Advisories {
        #[arg(long)]
        ecosystem: String,
    },
}

#[derive(Subcommand)]
enum GovtAction {
    /// Search Congressional bills
    Bills {
        #[arg(long)]
        query: String,
        #[arg(long)]
        congress: Option<u32>,
    },
    /// Search Federal Register regulations
    Regulations {
        #[arg(long)]
        query: String,
    },
}

#[derive(Subcommand)]
enum PoliticsAction {
    /// Search FEC candidates
    FecCandidates {
        #[arg(long)]
        query: String,
    },
    /// Search FEC committees
    FecCommittees {
        #[arg(long)]
        query: String,
    },
}

#[derive(Subcommand)]
enum PatentsAction {
    /// Search USPTO patents
    Search {
        #[arg(long)]
        query: String,
    },
    /// Get patent details by ID
    Details {
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
enum SatelliteAction {
    /// Query NASA FIRMS active fire data
    FirmsFires {
        #[arg(long)]
        lat: f64,
        #[arg(long)]
        lon: f64,
        #[arg(long)]
        radius_km: Option<f64>,
    },
}

#[derive(Subcommand)]
enum EnvAction {
    /// Search EPA regulated facilities
    EpaFacilities {
        #[arg(long)]
        query: String,
    },
    /// Search EPA Toxic Release Inventory emissions
    EpaEmissions {
        #[arg(long)]
        query: String,
    },
}

#[derive(Subcommand)]
enum LegalAction {
    /// Search US court cases
    SearchCases {
        #[arg(long)]
        query: String,
    },
    /// Get case details by ID
    CaseDetails {
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
enum HealthAction {
    /// CDC leading causes of death
    CdcLeadingCauses {
        #[arg(long)]
        state: String,
        #[arg(long)]
        year: Option<u32>,
    },
    /// WHO Global Health Observatory data
    WhoGho {
        #[arg(long)]
        indicator: String,
    },
}

#[derive(Subcommand)]
enum ClimateAction {
    /// NOAA historical weather observations
    NoaaObservations {
        #[arg(long)]
        location: String,
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
    },
    /// Find NOAA weather stations
    NoaaStations {
        #[arg(long)]
        location: String,
    },
}

#[derive(Subcommand)]
enum InsightsAction {
    /// Find cross-domain entity connections
    FindConnections {
        #[arg(long)]
        entity: Option<String>,
        #[arg(long)]
        min_domains: Option<i32>,
        #[arg(long)]
        limit: Option<i32>,
    },
    /// Detect trending entities
    TrendingEntities {
        #[arg(long)]
        time_window_hours: Option<i64>,
        #[arg(long)]
        min_growth: Option<f64>,
        #[arg(long)]
        min_current_mentions: Option<u32>,
    },
    /// Index articles from a JSON file or stdin
    IndexArticles {
        /// JSON file with articles, or - for stdin
        #[arg(long)]
        input: String,
    },
    /// Get insight engine statistics
    Stats,
    /// Clear all indexed articles
    ClearIndex,
}

#[derive(Subcommand)]
enum SopAction {
    /// List available SOP chains
    List,
    /// Execute a SOP chain
    Execute {
        #[arg(long)]
        chain: String,
        /// Query to substitute for $QUERY placeholder
        #[arg(long)]
        query: Option<String>,
        /// Target URL to substitute for $TARGET_URL placeholder
        #[arg(long)]
        target_url: Option<String>,
        /// Country code to substitute for $COUNTRY placeholder
        #[arg(long)]
        country: Option<String>,
    },
}

#[derive(Subcommand)]
enum IntelligenceAction {
    /// Summarize text using TextRank
    Summarize {
        /// Text to summarize, or - for stdin
        #[arg(long)]
        text: String,
        /// Number of sentences (default: 3)
        #[arg(long)]
        num_sentences: Option<u32>,
    },
    /// Resolve entity names to canonical forms
    ResolveEntities {
        /// Comma-separated entity names
        #[arg(long, value_delimiter = ',')]
        names: Vec<String>,
    },
    /// Search GDELT global events database
    Gdelt {
        #[arg(long)]
        query: String,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        start_date: Option<String>,
        #[arg(long)]
        end_date: Option<String>,
    },
}

#[derive(Subcommand)]
enum AdvancedAction {
    /// Analyze time series for anomalies
    TemporalAnalysis {
        #[arg(long)]
        entity: String,
        /// JSON array of [timestamp, count] pairs, or - for stdin
        #[arg(long)]
        points: String,
    },
    /// Extract geographic locations from text
    ExtractLocations {
        /// Text to analyze, or - for stdin
        #[arg(long)]
        text: String,
    },
    /// Detect the language of text
    DetectLanguage {
        /// Text to analyze, or - for stdin
        #[arg(long)]
        text: String,
    },
    /// Score source quality and trustworthiness
    SourceQuality {
        /// JSON array of [name, domain] pairs, or - for stdin
        #[arg(long)]
        sources: String,
    },
    /// Generate a markdown intelligence report
    GenerateReport {
        #[arg(long)]
        title: String,
        /// JSON array of articles, or - for stdin
        #[arg(long)]
        articles: String,
        #[arg(long, default_value = "brief")]
        style: String,
    },
    /// Semantic search over articles using TF-IDF
    SemanticSearch {
        #[arg(long)]
        query: String,
        /// JSON array of articles to search, or - for stdin
        #[arg(long)]
        articles: String,
        #[arg(long, default_value = "20")]
        limit: u32,
    },
}

#[derive(Subcommand)]
enum PluginsAction {
    /// Enrich articles via an external webhook
    WebhookEnrich {
        #[arg(long)]
        url: String,
        /// JSON array of articles, or - for stdin
        #[arg(long)]
        articles: String,
    },
    /// Pipe text through an external script
    ScriptHook {
        /// Script command (e.g., "python3 enrich.py")
        #[arg(long)]
        command: String,
        /// Text to pipe, or - for stdin
        #[arg(long)]
        text: String,
    },
    /// Export data to a file
    Export {
        /// JSON data to export, or - for stdin
        #[arg(long)]
        data: String,
        #[arg(long)]
        file: String,
        #[arg(long, default_value = "json")]
        format: String,
    },
}

#[derive(Subcommand)]
enum OsintAction {
    /// Search OpenAlex for academic works
    OpenAlex {
        #[arg(long)]
        query: String,
        #[arg(long, default_value = "25")]
        limit: u32,
    },
    /// Search Shodan for exposed services
    Shodan {
        #[arg(long)]
        query: String,
        #[arg(long)]
        api_key: String,
    },
    /// Check email against HaveIBeenPwned
    Hibp {
        #[arg(long)]
        email: String,
        #[arg(long)]
        api_key: String,
    },
    /// Search ACLED for conflict events
    Acled {
        #[arg(long)]
        country: Option<String>,
        #[arg(long)]
        event_type: Option<String>,
        #[arg(long)]
        start_date: Option<String>,
        #[arg(long)]
        end_date: Option<String>,
        #[arg(long)]
        api_key: String,
        #[arg(long)]
        email: String,
    },
}

#[derive(Subcommand)]
enum MonitorAction {
    /// Create a new monitor
    Create {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, value_delimiter = ',')]
        pools: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        keywords: Vec<String>,
        #[arg(long, default_value = "300")]
        interval_secs: u64,
        #[arg(long, default_value = "1")]
        threshold: u32,
        #[arg(long)]
        webhook_url: Option<String>,
        #[arg(long)]
        alert_file: Option<String>,
    },
    /// List all monitors
    List,
    /// Delete a monitor
    Delete {
        #[arg(long)]
        id: String,
    },
    /// Pause a monitor
    Pause {
        #[arg(long)]
        id: String,
    },
    /// Resume a paused monitor
    Resume {
        #[arg(long)]
        id: String,
    },
    /// Test a notification channel
    Test {
        /// Channel to test: slack, discord, telegram, email, webhook
        #[arg(long)]
        channel: String,
        /// Webhook URL (for slack, discord, email, webhook)
        #[arg(long)]
        url: Option<String>,
        /// Telegram bot token
        #[arg(long)]
        telegram_token: Option<String>,
        /// Telegram chat ID
        #[arg(long)]
        telegram_chat_id: Option<String>,
        /// Test message
        #[arg(long)]
        message: Option<String>,
    },
}

/// Convert Result<T, String> to anyhow::Result<T>
fn r<T>(result: Result<T, String>) -> anyhow::Result<T> {
    result.map_err(|e| anyhow::anyhow!(e))
}

fn output<T: serde::Serialize>(format: &str, value: &T) {
    let text = helpers::format_text(value, format);
    println!("{}", text);
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();
    let fmt = &cli.format;

    match cli.command {
        Commands::Mcp => {
            // MCP server mode — takes over stdin/stdout, no CLI output
            let settings = igs_rust_mcp::config::load_settings().await?;
            let tool_groups = settings.tool_groups.unwrap_or_default();
            let server = IgsMcpServer::new_with_groups(tool_groups);
            let service = server
                .serve(rmcp::transport::stdio())
                .await
                .inspect_err(|e| {
                    tracing::error!("MCP server error: {:?}", e);
                })?;
            service.waiting().await?;
            return Ok(());
        }

        Commands::Status => {
            let settings = igs_rust_mcp::config::load_settings().await?;
            println!("IGS Intelligence Gathering System");
            println!("  Version: {}", env!("CARGO_PKG_VERSION"));
            println!(
                "  Config:  {}",
                igs_rust_mcp::config::user_config_dir().display()
            );
            println!(
                "  HTTP:    timeout={}ms, retries={}, concurrency={}",
                settings.http.timeout_ms, settings.http.retries, settings.http.concurrency
            );
            println!(
                "  Cache:   enabled={}, ttl={}ms",
                settings.cache.enabled, settings.cache.ttl_ms
            );
            println!(
                "  NLP:     enabled={}, max_topics={}",
                settings.nlp.enabled, settings.nlp.max_topics
            );
            println!("  Obscura: enabled={}", settings.obscura.enabled);
            println!("  Output:  format={}", settings.output.default_format);

            let pools = igs_rust_mcp::config::load_pools().await?;
            let sources = igs_rust_mcp::config::load_sources().await?;
            println!("  Pools:   {}", pools.pools.len());
            println!("  Sources: {}", sources.sources.len());
        }

        Commands::Parsers => {
            let result = r(parsers_tools::parsers_list().await)?;
            output(fmt, &result);
        }

        Commands::ToolGroups { group } => {
            if let Some(group_name) = group {
                match registry::get_group_tools(&group_name) {
                    Some(tools) => {
                        let result = serde_json::json!({
                            "group": group_name,
                            "tool_count": tools.len(),
                            "tools": tools,
                        });
                        output(fmt, &result);
                    }
                    None => {
                        return Err(anyhow::anyhow!(
                            "Unknown group '{}'. Available groups: {}",
                            group_name,
                            registry::list_groups()
                                .iter()
                                .map(|(n, _)| *n)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            } else {
                let groups: Vec<_> = registry::TOOL_GROUPS
                    .iter()
                    .map(|g| {
                        serde_json::json!({
                            "name": g.name,
                            "description": g.description,
                            "tool_count": g.tools.len(),
                            "tools": g.tools,
                        })
                    })
                    .collect();
                let result = serde_json::json!({
                    "total_groups": groups.len(),
                    "total_tools": registry::total_tool_count(),
                    "groups": groups,
                });
                output(fmt, &result);
            }
        }

        Commands::Pools { action } => match action {
            PoolAction::List => {
                let result = r(pools::pools_list().await)?;
                output(fmt, &result);
            }
            PoolAction::Upsert {
                id,
                name,
                description,
            } => {
                let result = r(pools::pools_upsert(PoolUpsertInput {
                    id,
                    name,
                    description,
                    is_active: Some(true),
                })
                .await)?;
                output(fmt, &result);
            }
            PoolAction::Delete { id } => {
                let result = r(pools::pools_delete(PoolDeleteInput { id }).await)?;
                output(fmt, &result);
            }
        },

        Commands::Sources { action } => match action {
            SourceAction::List { pool, active_only } => {
                let pools = pool.map(|p| vec![p]);
                let result = r(sources::sources_list(SourceListInput {
                    pools,
                    active_only: Some(active_only),
                    cursor: None,
                    page_size: None,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            SourceAction::Upsert {
                name,
                source_type,
                url,
                pools,
                countries,
                cities,
                domains,
                parser,
                active,
            } => {
                let result = r(sources::sources_upsert(SourceUpsertInput {
                    id: None,
                    name,
                    source_type,
                    url,
                    headers: None,
                    parser,
                    pools,
                    countries,
                    cities,
                    domains,
                    is_active: active,
                })
                .await)?;
                output(fmt, &result);
            }
            SourceAction::Delete { id } => {
                let result = r(sources::sources_delete(SourceDeleteInput { id }).await)?;
                output(fmt, &result);
            }
            SourceAction::EnableScraper {
                id,
                list_url,
                item_selector,
                title_selector,
                link_selector,
                date_selector,
            } => {
                let mut selectors = std::collections::HashMap::new();
                if let Some(s) = item_selector {
                    selectors.insert("item".into(), s);
                }
                if let Some(s) = title_selector {
                    selectors.insert("title".into(), s);
                }
                if let Some(s) = link_selector {
                    selectors.insert("link".into(), s);
                }
                if let Some(s) = date_selector {
                    selectors.insert("date".into(), s);
                }
                let result = r(sources::sources_enable_scraper(EnableScraperInput {
                    id,
                    list_url,
                    selectors: if selectors.is_empty() { None } else { Some(selectors) },
                })
                .await)?;
                output(fmt, &result);
            }
            SourceAction::Discover { url, pool, name } => {
                let pools = pool.map(|p| vec![p]);
                let result =
                    r(sources::sources_autodiscover(AutodiscoverInput { url, pools, name }).await)?;
                output(fmt, &result);
            }
            SourceAction::Countries => {
                let result = r(sources::sources_countries().await)?;
                output(fmt, &result);
            }
            SourceAction::Cities => {
                let result = r(sources::sources_cities().await)?;
                output(fmt, &result);
            }
            SourceAction::Domains => {
                let result = r(sources::sources_domains().await)?;
                output(fmt, &result);
            }
        },

        Commands::News { action } => match action {
            NewsAction::Fetch {
                pools,
                sources: srcs,
                countries,
                cities,
                domains,
                start,
                end,
                keywords,
                exclude_keywords,
                match_all,
                limit,
                cache_mode,
                depth,
                skip_enrich,
                skip_index,
            } => {
                let kw = keywords.map(KeywordFilter::Multiple);
                let result = r(news::news_fetch(NewsFetchInput {
                    filters: DiscoveryFilters {
                        pools,
                        sources: srcs,
                        countries,
                        cities,
                        domains,
                        start,
                        end,
                        keywords: kw,
                        exclude_keywords,
                        match_all: Some(match_all),
                        limit: Some(limit),
                        cache_mode: Some(cache_mode),
                    },
                    discovery_mode: None,
                    urgency: None,
                    skip_enrich: Some(skip_enrich),
                    skip_index: Some(skip_index),
                    depth_opts: DepthOptions { depth },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            NewsAction::Test { id, cache_mode } => {
                let result = r(news::news_test_source(NewsTestInput {
                    id,
                    cache_mode: Some(cache_mode),
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            NewsAction::Enrich { input, extract } => {
                let items_json = if let Some(path) = input {
                    if path == "-" {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                        buf
                    } else {
                        std::fs::read_to_string(&path)?
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Provide --input <file> or --input - for stdin"
                    ));
                };
                let items: Vec<EnrichItemInput> = serde_json::from_str(&items_json)?;
                let result = r(news::news_enrich(NewsEnrichInput {
                    items,
                    extract,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Reddit { action } => match action {
            RedditAction::Search {
                query,
                subreddits,
                sort,
                time,
                limit,
            } => {
                let result = r(reddit::reddit_search(RedditSearchInput {
                    query,
                    subreddits,
                    sort: Some(sort),
                    time: Some(time),
                    limit: Some(limit),
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            RedditAction::Feed { subreddits, limit } => {
                let result = r(reddit::reddit_feed(RedditFeedInput {
                    subreddits,
                    limit: Some(limit),
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Research { action } => match action {
            ResearchAction::Search {
                query,
                sources: srcs,
                categories,
                year_from,
                year_to,
                limit,
            } => {
                let result = r(research::research_search(ResearchSearchInput {
                    query,
                    sources: Some(srcs),
                    categories,
                    year_from,
                    year_to,
                    limit: Some(limit),
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            ResearchAction::Paper {
                id,
                include_citations,
                include_references,
            } => {
                let result = r(research::research_paper(ResearchPaperInput {
                    paper_id: id,
                    include_citations: Some(include_citations),
                    include_references: Some(include_references),
                    extract_pdf: None,
                })
                .await)?;
                output(fmt, &result);
            }
            ResearchAction::Download {
                id,
                output: out,
                convert_to_markdown,
            } => {
                let result = r(research::research_download(ResearchDownloadInput {
                    paper_id: id,
                    output_path: out,
                    output: OutputOptions { format: None },
                    convert_to_markdown: Some(convert_to_markdown),
                })
                .await)?;
                output(fmt, &result);
            }
            ResearchAction::PubMedSearch { query, limit } => {
                let result = r(research::research_pubmed_search(ResearchPubMedInput {
                    query,
                    limits: LimitInput { limit: Some(limit as u32) },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Web { action } => match action {
            WebAction::Search {
                query,
                max_results,
                topic,
                include_domains,
                exclude_domains,
                provider,
                days,
                include_answer,
            } => {
                let result = r(web::web_search(WebSearchInput {
                    query,
                    provider,
                    max_results: Some(max_results),
                    topic,
                    include_domains,
                    exclude_domains,
                    days,
                    include_answer: Some(include_answer),
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            WebAction::Scrape {
                url,
                provider,
                wait_selector,
                strip_mode,
                wait_until,
                include_frames,
            } => {
                let result = r(web::web_scrape(WebScrapeInput {
                    url,
                    provider: Some(provider),
                    formats: None,
                    wait_selector,
                    strip_mode,
                    structured_data: None,
                    include_frames: Some(include_frames),
                    wait_until,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            WebAction::Crawl {
                url,
                max_depth,
                max_pages,
                obey_robots,
                dump_format,
                wait_selector,
            } => {
                let result = r(web::web_crawl(WebCrawlInput {
                    url,
                    provider: None,
                    max_depth: Some(max_depth),
                    max_pages: Some(max_pages),
                    obey_robots: Some(obey_robots),
                    dump_format: Some(dump_format),
                    wait_until: None,
                    include_frames: None,
                    wait_selector,
                    strip_mode: None,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            WebAction::Map { url, limit, search } => {
                let result = r(web::web_map(WebMapInput {
                    url,
                    provider: None,
                    limit: Some(limit),
                    search,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Twitter { action } => match action {
            TwitterAction::Search { query, limit, mode } => {
                let result = r(twitter::twitter_search(TwitterSearchInput {
                    query,
                    limit: Some(limit as u32),
                    search_mode: mode,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            TwitterAction::Read { url } => {
                let result = r(twitter::twitter_read(TwitterReadInput {
                    url,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Youtube { action } => match action {
            YoutubeAction::Search { query, limit } => {
                let result = r(youtube::youtube_search(YoutubeSearchInput {
                    query,
                    limit: Some(limit as u32),
                })
                .await)?;
                output(fmt, &result);
            }
            YoutubeAction::Metadata { url } => {
                let result = r(youtube::youtube_metadata(YoutubeMetadataInput { url }).await)?;
                output(fmt, &result);
            }
            YoutubeAction::Subtitles { url, lang } => {
                let result = r(youtube::youtube_subtitles(YoutubeSubtitlesInput {
                    url,
                    language: lang,
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Browser { action } => {
            match action {
                BrowserAction::Goto { url, wait_until } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_goto(
                        LpGotoInput {
                            url,
                            wait_until: Some(wait_until),
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Markdown { strip_mode } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_markdown(
                        LpMarkdownInput { strip_mode },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Links { selector } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_links(
                        LpLinksInput { selector },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Evaluate { expression } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_evaluate(
                        LpEvaluateInput { expression },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Click {
                    selector,
                    wait_for_navigation,
                } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_click(
                        LpClickInput {
                            selector,
                            wait_for_navigation: Some(wait_for_navigation),
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Fill { selector, value } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_fill(
                        LpFillInput { selector, value },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Scroll { direction, pixels } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_scroll(
                        LpScrollInput {
                            direction: Some(direction),
                            pixels: Some(pixels),
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::WaitForSelector {
                    selector,
                    timeout_ms,
                } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_wait_for_selector(
                        LpWaitForSelectorInput {
                            selector,
                            timeout_ms: Some(timeout_ms),
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
            }
        }

        Commands::Weather { action } => match action {
            WeatherAction::Forecast { location, days } => {
                let result = r(weather::weather_forecast(WeatherForecastInput {
                    location,
                    days,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            WeatherAction::Current { location } => {
                let result = r(weather::weather_current(WeatherCurrentInput {
                    location,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            WeatherAction::Alerts { lat, lon } => {
                let result = r(weather::weather_alerts(WeatherAlertsInput {
                    latitude: lat,
                    longitude: lon,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Finance { action } => match action {
            FinanceAction::Market { symbols } => {
                let result = r(finance::finance_market(FinanceMarketInput {
                    symbols,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            FinanceAction::Crypto { symbols } => {
                let result = r(finance::finance_crypto(FinanceCryptoInput {
                    symbols,
                    ids: vec![],
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            FinanceAction::Trending => {
                let result = r(finance::finance_trending(FinanceTrendingInput {
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Security { action } => match action {
            SecurityAction::Cve { query, days_back } => {
                let result = r(security::security_cve_search(CveSearchInput {
                    query,
                    severity: None,
                    days_back: days_back.map(|d| d as u32),
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            SecurityAction::Advisories { ecosystem } => {
                let result = r(security::security_advisories(SecurityAdvisoriesInput {
                    ecosystem,
                    severity: None,
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Govt { action } => match action {
            GovtAction::Bills { query, congress } => {
                let result = r(govt::govt_bills(GovtBillsInput {
                    query,
                    congress,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            GovtAction::Regulations { query } => {
                let result = r(govt::govt_regulations(GovtRegulationsInput {
                    query,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Politics { action } => match action {
            PoliticsAction::FecCandidates { query } => {
                let result = r(politics::politics_fec_candidates(PoliticsFecInput {
                    name: query,
                    office: None,
                    party: None,
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            PoliticsAction::FecCommittees { query } => {
                let result = r(politics::politics_fec_committees(PoliticsFecCommitteesInput {
                    name: query,
                    committee_type: None,
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Patents { action } => match action {
            PatentsAction::Search { query } => {
                let result = r(patents::patents_search(PatentSearchInput {
                    query,
                    office: None,
                    years_back: None,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            PatentsAction::Details { id } => {
                let result = r(patents::patents_details(PatentDetailsInput {
                    patent_id: id,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Satellite { action } => match action {
            SatelliteAction::FirmsFires { lat, lon, radius_km } => {
                // Convert lat/lon + radius to bounding box (west, south, east, north)
                let radius = radius_km.unwrap_or(50.0);
                let lat_offset = radius / 111.0;
                let lon_offset = radius / (111.0 * lat.to_radians().cos().abs().max(0.01));
                let result = r(satellite::satellite_firms_fires(SatelliteFirmsInput {
                    west: lon - lon_offset,
                    south: lat - lat_offset,
                    east: lon + lon_offset,
                    north: lat + lat_offset,
                    source: None,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Env { action } => match action {
            EnvAction::EpaFacilities { query } => {
                let result = r(env::env_epa_facilities(EnvEpaFacilitiesInput {
                    state: None,
                    name: Some(query),
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            EnvAction::EpaEmissions { query } => {
                let result = r(env::env_epa_emissions(EnvEpaEmissionsInput {
                    state: Some(query),
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Legal { action } => match action {
            LegalAction::SearchCases { query } => {
                let result = r(legal::legal_search_cases(LegalSearchInput {
                    query,
                    court: None,
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            LegalAction::CaseDetails { id } => {
                let case_id: u32 = id.parse().map_err(|e| anyhow::anyhow!("Invalid case ID (must be numeric): {}", e))?;
                let result = r(legal::legal_case_details(LegalCaseDetailsInput {
                    case_id,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Health { action } => match action {
            HealthAction::CdcLeadingCauses { state, year } => {
                let result = r(health::health_cdc_leading_causes(HealthCdcInput {
                    state: Some(state),
                    year,
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            HealthAction::WhoGho { indicator } => {
                let result = r(health::health_who_gho(HealthWhoInput {
                    indicator: Some(indicator),
                    country: None,
                    year: None,
                    limits: LimitInput { limit: None },
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Climate { action } => match action {
            ClimateAction::NoaaObservations { location, start, end } => {
                let result = r(climate::climate_noaa_observations(ClimateNoaaInput {
                    dataset: None,
                    location: Some(location),
                    station: None,
                    start_date: start,
                    end_date: end,
                    limit: None,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
            ClimateAction::NoaaStations { location } => {
                let result = r(climate::climate_noaa_stations(ClimateNoaaStationsInput {
                    location: Some(location),
                    limit: None,
                    output: OutputOptions { format: None },
                })
                .await)?;
                output(fmt, &result);
            }
        },

        Commands::Insights { action } => {
            // Insights tools require the shared InsightStorage from the server.
            // For CLI use, we create a standalone server instance to access
            // the insight engine.
            let server = IgsMcpServer::new();
            match action {
                InsightsAction::FindConnections {
                    entity,
                    min_domains,
                    limit,
                } => {
                    let result = r(insights::insights_find_connections(
                        &server.insights(),
                        InsightFindConnectionsInput {
                            entity,
                            min_domains,
                            limit,
                            output: OutputOptions { format: None },
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                InsightsAction::TrendingEntities {
                    time_window_hours,
                    min_growth,
                    min_current_mentions,
                } => {
                    let result = r(insights::insights_trending(
                        &server.insights(),
                        InsightTrendingInput {
                            time_window_hours,
                            min_growth,
                            min_current_mentions,
                            output: OutputOptions { format: None },
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                InsightsAction::IndexArticles { input: input_path } => {
                    let items_json = if input_path == "-" {
                        let mut buf = String::new();
                        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                        buf
                    } else {
                        std::fs::read_to_string(&input_path)?
                    };
                    let articles: Vec<InsightIndexArticle> = serde_json::from_str(&items_json)?;
                    let result = r(insights::insights_index(
                        &server.insights(),
                        InsightIndexInput { articles },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                InsightsAction::Stats => {
                    let result = r(insights::insights_stats(&server.insights()).await)?;
                    output(fmt, &result);
                }
                InsightsAction::ClearIndex => {
                    let result = r(insights::insights_clear(&server.insights()).await)?;
                    output(fmt, &result);
                }
            }
        }

        Commands::Sop { action } => match action {
            SopAction::List => {
                let result = sop::sop_list();
                output(fmt, &result);
            }
            SopAction::Execute { chain, query, target_url, country } => {
                let result = r(sop::sop_execute(SopExecuteInput {
                    chain_name: chain,
                    query,
                    target_url,
                    country,
                    output: OutputOptions { format: None },
                }))?;
                output(fmt, &result);
            }
        },

        Commands::Monitor { action } => {
            use igs_rust_mcp::tools::monitor::{MonitorConfig, MonitorManager};
            let settings = igs_rust_mcp::config::load_settings().await
                .map_err(|e| anyhow::anyhow!("Settings load failed: {}", e))?;
            let manager = MonitorManager::new(std::sync::Arc::new(settings));
            match action {
                MonitorAction::Create {
                    id,
                    name,
                    pools,
                    keywords,
                    interval_secs,
                    threshold,
                    webhook_url,
                    alert_file,
                } => {
                    manager.add(MonitorConfig {
                        id: id.clone(),
                        name,
                        pools,
                        keywords,
                        interval_secs,
                        threshold,
                        webhook_url,
                        webhook_format: None,
                        alert_file,
                        telegram_bot_token: None,
                        telegram_chat_id: None,
                        email_webhook_url: None,
                        email_recipients: None,
                        cooldown_secs: None,
                        active: true,
                    }).await;
                    output(fmt, &MonitorCreateOutput { created: true, id });
                }
                MonitorAction::List => {
                    let monitors = manager.list().await;
                    let count = monitors.len();
                    let monitors: Vec<MonitorInfo> = monitors
                        .into_iter()
                        .map(|m| MonitorInfo {
                            id: m.id,
                            name: m.name,
                            pools: m.pools,
                            keywords: m.keywords,
                            interval_secs: m.interval_secs,
                            threshold: m.threshold,
                            active: m.active,
                        })
                        .collect();
                    output(fmt, &MonitorListOutput { monitors, count });
                }
                MonitorAction::Delete { id } => {
                    let removed = manager.remove(&id).await;
                    output(fmt, &MonitorDeleteOutput { removed });
                }
                MonitorAction::Pause { id } => {
                    let paused = manager.pause(&id).await;
                    output(fmt, &MonitorPauseOutput { paused });
                }
                MonitorAction::Resume { id } => {
                    let resumed = manager.resume(&id).await;
                    output(fmt, &MonitorPauseOutput { paused: !resumed });
                }
                MonitorAction::Test { channel, url, telegram_token, telegram_chat_id, message } => {
                    let result = manager.test_alert(igs_rust_mcp::tools::monitor::MonitorTestInput {
                        channel,
                        webhook_url: url,
                        telegram_bot_token: telegram_token,
                        telegram_chat_id,
                        message,
                    }).await;
                    output(fmt, &result);
                }
            }
        }

        Commands::Intelligence { action } => match action {
            IntelligenceAction::Summarize { text, num_sentences } => {
                let text = if text == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    text
                };
                let num = num_sentences.unwrap_or(3) as usize;
                let result = igs_rust_mcp::tools::summarize::summarize(&text, num);
                output(fmt, &SummarizeOutput {
                    summary: result.summary,
                    sentence_count: result.sentence_count,
                    original_count: result.original_count,
                    top_sentences: result.top_sentences,
                });
            }
            IntelligenceAction::ResolveEntities { names } => {
                let result = igs_rust_mcp::tools::entity_resolution::resolve_entities(&names);
                output(fmt, &result);
            }
            IntelligenceAction::Gdelt { query, limit, start_date, end_date } => {
                let result = r(igs_rust_mcp::tools::gdelt::gdelt_search(
                    igs_rust_mcp::tools::gdelt::GdeltSearchInput {
                        query,
                        limit,
                        start_date,
                        end_date,
                        limits: LimitInput { limit: None },
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
        },

        Commands::Advanced { action } => match action {
            AdvancedAction::TemporalAnalysis { entity, points } => {
                let points_str = if points == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    points
                };
                let pts: Vec<(String, u32)> = serde_json::from_str(&points_str)
                    .map_err(|e| anyhow::anyhow!("Invalid points JSON: {}", e))?;
                let result = igs_rust_mcp::tools::advanced::analyze_time_series(&entity, &pts);
                output(fmt, &result);
            }
            AdvancedAction::ExtractLocations { text } => {
                let text = if text == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    text
                };
                let result = igs_rust_mcp::tools::advanced::extract_locations(&text);
                output(fmt, &result);
            }
            AdvancedAction::DetectLanguage { text } => {
                let text = if text == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    text
                };
                let result = igs_rust_mcp::tools::advanced::detect_language(&text);
                output(fmt, &result);
            }
            AdvancedAction::SourceQuality { sources } => {
                let sources_str = if sources == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    sources
                };
                let srcs: Vec<(String, String)> = serde_json::from_str(&sources_str)
                    .map_err(|e| anyhow::anyhow!("Invalid sources JSON: {}", e))?;
                let result = igs_rust_mcp::tools::advanced::score_sources(&srcs);
                output(fmt, &result);
            }
            AdvancedAction::GenerateReport { title, articles, style } => {
                let articles_str = if articles == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    articles
                };
                let arts: Vec<igs_rust_mcp::tools::advanced::ReportArticle> = serde_json::from_str(&articles_str)
                    .map_err(|e| anyhow::anyhow!("Invalid articles JSON: {}", e))?;
                let result = igs_rust_mcp::tools::advanced::generate_report(
                    igs_rust_mcp::tools::advanced::ReportInput {
                        title,
                        articles: arts,
                        summary_style: Some(style),
                    },
                );
                output(fmt, &result);
            }
            AdvancedAction::SemanticSearch { query, articles, limit } => {
                let articles_str = if articles == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    articles
                };
                // Parse articles as Vec of (id, title, link, text) tuples
                let article_tuples: Vec<(String, String, String, String)> = serde_json::from_str(&articles_str)
                    .map_err(|e| anyhow::anyhow!("Invalid articles JSON: {}", e))?;
                let mut index = igs_rust_mcp::tools::semantic::SemanticIndex::new();
                index.add_batch(&article_tuples);
                let results = index.search(&query, limit as usize);
                let count = results.len();
                output(fmt, &igs_rust_mcp::tools::semantic::SemanticSearchOutput {
                    query,
                    results,
                    count,
                });
            }
        },

        Commands::Plugins { action } => match action {
            PluginsAction::WebhookEnrich { url, articles } => {
                let articles_str = if articles == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    articles
                };
                let result = r(igs_rust_mcp::tools::plugins::webhook_enrich(
                    igs_rust_mcp::tools::plugins::WebhookEnrichInput {
                        webhook_url: url,
                        articles_json: articles_str,
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
            PluginsAction::ScriptHook { command, text } => {
                let text = if text == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    text
                };
                let result = r(igs_rust_mcp::tools::plugins::script_hook(
                    igs_rust_mcp::tools::plugins::ScriptHookInput {
                        command,
                        text,
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
            PluginsAction::Export { data, file, format } => {
                let data_str = if data == "-" {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                } else {
                    data
                };
                let result = r(igs_rust_mcp::tools::plugins::export_data(
                    igs_rust_mcp::tools::plugins::ExportInput {
                        data_json: data_str,
                        file_path: file,
                        format: Some(format),
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
        },

        Commands::Osint { action } => match action {
            OsintAction::OpenAlex { query, limit } => {
                let result = r(igs_rust_mcp::tools::data_sources::openalex_search(
                    igs_rust_mcp::tools::data_sources::OpenAlexSearchInput {
                        query,
                        limit: Some(limit),
                        limits: LimitInput { limit: None },
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
            OsintAction::Shodan { query, api_key } => {
                let result = r(igs_rust_mcp::tools::data_sources::shodan_search(
                    igs_rust_mcp::tools::data_sources::ShodanSearchInput {
                        query,
                        api_key,
                        limits: LimitInput { limit: None },
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
            OsintAction::Hibp { email, api_key } => {
                let result = r(igs_rust_mcp::tools::data_sources::hibp_check(
                    igs_rust_mcp::tools::data_sources::HibpBreachInput {
                        email,
                        api_key,
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
            OsintAction::Acled { country, event_type, start_date, end_date, api_key, email } => {
                let result = r(igs_rust_mcp::tools::data_sources::acled_search(
                    igs_rust_mcp::tools::data_sources::AcledSearchInput {
                        country,
                        event_type,
                        start_date,
                        end_date,
                        api_key,
                        email,
                        limits: LimitInput { limit: None },
                        output: OutputOptions { format: None },
                    },
                ).await)?;
                output(fmt, &result);
            }
        },
    }

    Ok(())
}
