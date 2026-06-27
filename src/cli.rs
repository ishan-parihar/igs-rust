use clap::{Parser, Subcommand};
use igs_rust_mcp::server::IgsMcpServer;
use igs_rust_mcp::tools::types::*;
use igs_rust_mcp::tools::types_base::{
    DepthOptions, DiscoveryFilters, KeywordFilter, OutputOptions,
};
use igs_rust_mcp::tools::{
    helpers::toon_encode, news, parsers as parsers_tools, pools, reddit, registry, research,
    sources, twitter, web, youtube,
};
use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::sync::Mutex;
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
        #[arg(long)]
        start: Option<String>,
        #[arg(long)]
        end: Option<String>,
        #[arg(long, value_delimiter = ',')]
        keywords: Option<Vec<String>>,
        #[arg(long, default_value = "50")]
        limit: i32,
        #[arg(long, default_value = "prefer")]
        cache_mode: String,
        /// Fetch depth: "quick" (direct RSS), "deep" (source site crawl), "full" (multi-source enrichment)
        #[arg(long)]
        depth: Option<String>,
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
    /// Get semantic DOM tree
    SemanticTree {
        #[arg(long)]
        include_text: bool,
    },
    /// Extract structured data (JSON-LD, OpenGraph)
    StructuredData,
    /// Detect forms on current page
    DetectForms {
        #[arg(long)]
        selector: Option<String>,
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
    /// Find interactive elements
    InteractiveElements {
        #[arg(long)]
        selector: Option<String>,
    },
}

/// Convert Result<T, String> to anyhow::Result<T>
fn r<T>(result: Result<T, String>) -> anyhow::Result<T> {
    result.map_err(|e| anyhow::anyhow!(e))
}

fn output<T: serde::Serialize>(format: &str, value: &T) {
    let text = if format == "json" {
        serde_json::to_string_pretty(value).unwrap_or_default()
    } else {
        toon_encode(value)
    };
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
                start,
                end,
                keywords,
                limit,
                cache_mode,
                depth,
            } => {
                let kw = keywords.map(KeywordFilter::Multiple);
                let result = r(news::news_fetch(NewsFetchInput {
                    filters: DiscoveryFilters {
                        pools,
                        sources: srcs,
                        countries,
                        cities: None,
                        domains: None,
                        start,
                        end,
                        keywords: kw,
                        exclude_keywords: None,
                        match_all: None,
                        limit: Some(limit),
                        cache_mode: Some(cache_mode),
                    },
                    discovery_mode: None,
                    urgency: None,
                    skip_enrich: None,
                    skip_index: None,
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
        },

        Commands::Web { action } => match action {
            WebAction::Search {
                query,
                max_results,
                topic,
                include_domains,
                exclude_domains,
            } => {
                let result = r(web::web_search(WebSearchInput {
                    query,
                    provider: None,
                    max_results: Some(max_results),
                    topic,
                    include_domains,
                    exclude_domains,
                    days: None,
                    include_answer: None,
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
            TwitterAction::Search {
                query,
                limit,
                mode,
            } => {
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
                let result = r(youtube::youtube_metadata(YoutubeMetadataInput {
                    url,
                })
                .await)?;
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
            let _settings = igs_rust_mcp::config::load_settings().await?;

            match action {
                BrowserAction::Goto { url, wait_until } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_goto(
                        &Arc::new(Mutex::new(None)),
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
                        &Arc::new(Mutex::new(None)),
                        LpMarkdownInput { strip_mode },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Links { selector } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_links(
                        &Arc::new(Mutex::new(None)),
                        LpLinksInput { selector },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Evaluate { expression } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_evaluate(
                        &Arc::new(Mutex::new(None)),
                        LpEvaluateInput { expression },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::SemanticTree { include_text } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_semantic_tree(
                        &Arc::new(Mutex::new(None)),
                        LpSemanticTreeInput {
                            include_text: Some(include_text),
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::StructuredData => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_structured_data(
                        &Arc::new(Mutex::new(None)),
                        LpStructuredDataInput {
                            jsonld: None,
                            opengraph: None,
                            microdata: None,
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::DetectForms { selector } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_detect_forms(
                        &Arc::new(Mutex::new(None)),
                        LpDetectFormsInput { selector },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Click {
                    selector,
                    wait_for_navigation,
                } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_click(
                        &Arc::new(Mutex::new(None)),
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
                        &Arc::new(Mutex::new(None)),
                        LpFillInput { selector, value },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::Scroll { direction, pixels } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_scroll(
                        &Arc::new(Mutex::new(None)),
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
                        &Arc::new(Mutex::new(None)),
                        LpWaitForSelectorInput {
                            selector,
                            timeout_ms: Some(timeout_ms),
                        },
                    )
                    .await)?;
                    output(fmt, &result);
                }
                BrowserAction::InteractiveElements { selector } => {
                    let result = r(igs_rust_mcp::tools::lp_mcp::lp_interactive_elements(
                        &Arc::new(Mutex::new(None)),
                        LpInteractiveElementsInput { selector },
                    )
                    .await)?;
                    output(fmt, &result);
                }
            }
        }
    }

    Ok(())
}
