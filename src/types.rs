use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── YAML Config Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Pool {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PoolsFile {
    pub pools: Vec<Pool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SourceParserConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selectors: Option<Selectors>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Selectors {
    pub item: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RateLimit {
    pub interval_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser_config: Option<SourceParserConfig>,
    #[serde(default)]
    pub pools: Vec<String>,
    #[serde(default)]
    pub countries: Vec<String>,
    #[serde(default)]
    pub cities: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SourcesFile {
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpSettings {
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default = "default_backoff_base")]
    pub backoff_base_ms: u64,
    #[serde(default = "default_backoff_factor")]
    pub backoff_factor: f64,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_per_host")]
    pub per_host: u32,
}

fn default_user_agent() -> String {
    "IGS/0.3.2 (+https://github.com/ishan-parihar/igs-rust-mcp)".to_string()
}
fn default_timeout() -> u64 {
    15000
}
fn default_retries() -> u32 {
    2
}
fn default_backoff_base() -> u64 {
    500
}
fn default_backoff_factor() -> f64 {
    2.0
}
fn default_concurrency() -> u32 {
    6
}
fn default_per_host() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CacheSettings {
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
    #[serde(default = "default_cache_dir")]
    pub dir: String,
    #[serde(default = "default_honor_etags")]
    pub honor_etags: bool,
    #[serde(default = "default_ttl_ms")]
    pub ttl_ms: u64,
    #[serde(default = "default_query_ttl_ms")]
    pub query_ttl_ms: u64,
    #[serde(default = "default_max_items")]
    pub max_items_per_source: u32,
}

fn default_cache_enabled() -> bool {
    true
}
fn default_cache_dir() -> String {
    "cache".to_string()
}
fn default_honor_etags() -> bool {
    true
}
fn default_ttl_ms() -> u64 {
    1_800_000
}
fn default_query_ttl_ms() -> u64 {
    600_000
}
fn default_max_items() -> u32 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeSettings {
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

fn default_timezone() -> String {
    "local".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TavilySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default = "default_tavily_depth")]
    pub search_depth: String,
    #[serde(default = "default_tavily_topic")]
    pub default_topic: String,
    #[serde(default = "default_tavily_timeout")]
    pub timeout_ms: u64,
}

fn default_tavily_depth() -> String {
    "basic".to_string()
}
fn default_tavily_topic() -> String {
    "general".to_string()
}
fn default_tavily_timeout() -> u64 {
    30000
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FirecrawlSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default = "default_firecrawl_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_formats")]
    pub default_formats: Vec<String>,
}

fn default_firecrawl_timeout() -> u64 {
    60000
}
fn default_formats() -> Vec<String> {
    vec!["markdown".to_string(), "html".to_string(), "screenshot".to_string(), "links".to_string()]
}

fn default_true() -> bool {
    true
}

fn default_lp_timeout() -> u64 {
    30000
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LightpandaSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    #[serde(default)]
    pub prefer_nightly: bool,
    #[serde(default = "default_true")]
    pub obey_robots: bool,
    #[serde(default = "default_lp_timeout")]
    pub timeout_ms: u64,
    /// HTTP proxy URL (e.g., "http://proxy:8080" or "http://user:pass@proxy:8080")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Bearer token for proxy authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_bearer_token: Option<String>,
    /// Full custom User-Agent string (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Suffix appended to Lightpanda's User-Agent header (used if user_agent not set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent_suffix: Option<String>,
    /// Max concurrent HTTP requests during page load
    #[serde(default = "default_lp_max_concurrent")]
    pub max_concurrent: u32,
    /// Max response size in bytes (0 = no limit)
    #[serde(default)]
    pub max_response_size: u64,
    /// Disable TLS host verification (for sites with bad certs)
    #[serde(default)]
    pub insecure_tls: bool,
    /// Enable stealth mode via injected anti-fingerprinting JavaScript
    #[serde(default)]
    pub stealth: bool,
    /// Optional path to a custom JS file to inject (--inject-script-file)
    #[serde(default)]
    pub stealth_script_path: Option<String>,
}

fn default_lp_max_concurrent() -> u32 {
    10
}

impl Default for LightpandaSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_update: true,
            prefer_nightly: false,
            obey_robots: true,
            timeout_ms: 30000,
            proxy: None,
            proxy_bearer_token: None,
            user_agent: None,
            user_agent_suffix: None,
            max_concurrent: 10,
            max_response_size: 0,
            insecure_tls: false,
            stealth: false,
            stealth_script_path: None,
        }
    }
}
fn default_max_topics() -> usize { 8 }
fn default_max_entities() -> usize { 20 }
fn default_min_entity_length() -> usize { 2 }
fn default_dedup_threshold() -> f64 { 0.3 }
fn default_pipeline_pool() -> String { "GLOBAL_TECH_CYBER".to_string() }
fn default_pipeline_limit() -> i32 { 50 }
fn default_output_format() -> String { "toon".to_string() }
fn default_toon_indent() -> usize { 2 }
fn default_max_items_per_response() -> i32 { 500 }
fn default_dump_enabled() -> bool { false }
fn default_dump_dir() -> String { "~/Documents/IGS".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NlpSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_topics")]
    pub max_topics: usize,
    #[serde(default = "default_max_entities")]
    pub max_entities: usize,
    #[serde(default = "default_min_entity_length")]
    pub min_entity_length: usize,
    #[serde(default = "default_dedup_threshold")]
    pub dedup_threshold: f64,
}

impl Default for NlpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_topics: 8,
            max_entities: 20,
            min_entity_length: 2,
            dedup_threshold: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineSettings {
    #[serde(default = "default_pipeline_pool")]
    pub default_pool: String,
    #[serde(default = "default_pipeline_limit")]
    pub default_limit: i32,
    #[serde(default = "default_true")]
    pub auto_index: bool,
    #[serde(default = "default_true")]
    pub persist_insights: bool,
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            default_pool: "GLOBAL_TECH_CYBER".to_string(),
            default_limit: 50,
            auto_index: true,
            persist_insights: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputSettings {
    #[serde(default = "default_output_format")]
    pub default_format: String,
    #[serde(default = "default_toon_indent")]
    pub toon_indent: usize,
    #[serde(default = "default_max_items_per_response")]
    pub max_items_per_response: i32,
    #[serde(default = "default_dump_enabled")]
    pub dump_enabled: bool,
    #[serde(default = "default_dump_dir")]
    pub dump_dir: String,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            default_format: "toon".to_string(),
            toon_indent: 2,
            max_items_per_response: 500,
            dump_enabled: false,
            dump_dir: "~/Documents/IGS".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Settings {
    pub http: HttpSettings,
    pub cache: CacheSettings,
    pub time: TimeSettings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tavily: Option<TavilySettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firecrawl: Option<FirecrawlSettings>,
    #[serde(default)]
    pub lightpanda: LightpandaSettings,
    #[serde(default)]
    pub nlp: NlpSettings,
    #[serde(default)]
    pub pipeline: PipelineSettings,
    #[serde(default)]
    pub output: OutputSettings,
    #[serde(default)]
    pub tool_groups: Option<Vec<String>>,
}

// ─── News Types ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub link: String,
    pub pub_date: String,
    pub source_name: String,
    pub pool_id: String,
    pub content_snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    /// Date extraction confidence: "high" (RFC3339/datetime attr), "medium" (text parsed), "low" (fallback to Utc::now)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_confidence: Option<String>,
    /// Freshness score 0.0-100.0 based on recency (exponential decay)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness_score: Option<f64>,
}

// ─── Cache Entry Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedCacheEntry {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    pub fetched_at: u64,
    pub items: Vec<NewsItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCacheEntry<T: Clone> {
    pub meta: QueryCacheMeta,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCacheMeta {
    pub key: String,
    pub at: u64,
    pub deps: std::collections::HashMap<String, u64>,
}

// ─── Research Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResearchPaper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: String,
    pub year: Option<i32>,
    pub citation_count: Option<i32>,
    pub pdf_url: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

// ─── Entity Types ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DomainInfo {
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SentimentResult {
    pub score: f64,
    pub comparative: f64,
    pub label: String,
}

// ─── Insight Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArticleInsight {
    pub id: String,
    pub title: String,
    pub pub_date: String,
    pub source_name: String,
    pub domains: Vec<DomainInfo>,
    pub entities: Vec<EntityInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityConnection {
    pub entity: String,
    pub entity_type: String,
    pub domains: Vec<DomainConnection>,
    pub connection_strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DomainConnection {
    pub domain: String,
    pub article_ids: Vec<String>,
    pub article_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TrendingEntity {
    pub entity: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub current_mentions: u32,
    pub previous_mentions: u32,
    pub growth: f64,
    pub normalized_growth: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InsightStats {
    pub total_articles: usize,
    pub total_entities: usize,
    pub total_domains: usize,
    pub avg_entities_per_article: f64,
    pub avg_domains_per_article: f64,
}
