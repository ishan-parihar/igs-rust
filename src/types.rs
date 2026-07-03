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
    /// Per-source weight for RRF fusion (default 1.0). Used by `fusion::weighted_rrf_fusion`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    // Removed YAGNI fields (defined, deserialized, never read):
    //   tier: Option<u32>            — never read by any code path
    //   rate_limit: Option<RateLimit> — RateLimit struct deleted; HttpClient has its own per-host semaphore
    //   source_category: Option<String> — never read
    //   trust_score: Option<f64>     — never read
    // These fields are still accepted in sources.yml (serde ignores unknown fields
    // by default), so existing config files continue to work without modification.
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
    "IGS/0.3.2 (+https://github.com/ishan-parihar/igs-rust)".to_string()

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
    #[serde(default = "default_ttl_ms")]
    pub ttl_ms: u64,
    // Removed YAGNI fields (defined, deserialized, never read):
    //   honor_etags: bool          — http.rs doesn't check this; etags are always honored when present
    //   query_ttl_ms: u64          — QueryCache was deleted; this was its TTL
    //   max_items_per_source: u32  — FeedCache hardcodes max_items=1000; this was never wired through
}

fn default_cache_enabled() -> bool {
    true
}
fn default_cache_dir() -> String {
    "cache".to_string()
}
fn default_ttl_ms() -> u64 {
    1_800_000
}

// TimeSettings struct removed — `timezone` field was defined, deserialized, but never
// read by any code path. The `time:` section in settings.yml is silently ignored.

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
    vec![
        "markdown".to_string(),
        "html".to_string(),
        "screenshot".to_string(),
        "links".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

fn default_lp_timeout() -> u64 {
    30000
}

fn default_lp_max_concurrent() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObscuraSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    #[serde(default = "default_true")]
    pub obey_robots: bool,
    #[serde(default = "default_lp_timeout")]
    pub timeout_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    #[serde(default = "default_lp_max_concurrent")]
    pub max_concurrent: u32,
    #[serde(default)]
    pub stealth: bool,
}

impl Default for ObscuraSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_update: true,
            obey_robots: true,
            timeout_ms: 30000,
            proxy: None,
            max_concurrent: 10,
            stealth: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenWeatherSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CourtListenerSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

fn default_max_topics() -> usize {
    8
}
fn default_max_entities() -> usize {
    20
}
fn default_output_format() -> String {
    "toon".to_string()
}
fn default_dump_enabled() -> bool {
    false
}
fn default_dump_dir() -> String {
    "~/Documents/IGS".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NlpSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_topics")]
    pub max_topics: usize,
    #[serde(default = "default_max_entities")]
    pub max_entities: usize,
    // Removed YAGNI fields (defined, deserialized, never read):
    //   min_entity_length: usize — never read by any code path
    //   dedup_threshold: f64    — never read (batch_similar uses hardcoded 0.3)
}

impl Default for NlpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_topics: 8,
            max_entities: 20,
        }
    }
}

// PipelineSettings struct removed — all 4 fields (default_pool, default_limit,
// auto_index, persist_insights) were defined, deserialized, but never read by any
// code path. The `pipeline:` section in settings.yml is silently ignored.

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputSettings {
    #[serde(default = "default_output_format")]
    pub default_format: String,
    #[serde(default = "default_dump_enabled")]
    pub dump_enabled: bool,
    #[serde(default = "default_dump_dir")]
    pub dump_dir: String,
    // Removed YAGNI fields (defined, deserialized, never read):
    //   toon_indent: usize           — never read (toon_format uses its own default)
    //   max_items_per_response: i32  — never read (limits are hardcoded per-tool)
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            default_format: "toon".to_string(),
            dump_enabled: false,
            dump_dir: "~/Documents/IGS".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoaaSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComtradeSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TwitterSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RedditSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Settings {
    pub http: HttpSettings,
    pub cache: CacheSettings,
    // Removed YAGNI field: time: TimeSettings — `timezone` was never read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tavily: Option<TavilySettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firecrawl: Option<FirecrawlSettings>,
    #[serde(default)]
    pub obscura: ObscuraSettings,
    #[serde(default)]
    pub nlp: NlpSettings,
    // Removed YAGNI field: pipeline: PipelineSettings — all fields were never read
    #[serde(default)]
    pub output: OutputSettings,
    #[serde(default)]
    pub tool_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openweather: Option<OpenWeatherSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noaa: Option<NoaaSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub courtlistener: Option<CourtListenerSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comtrade: Option<ComtradeSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter: Option<TwitterSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reddit: Option<RedditSettings>,
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
