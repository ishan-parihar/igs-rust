//! Base input types for shared fields across IGS tool categories.
//! Eliminates ~300 tokens of duplicated `format: Option<String>` across 15 structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Keyword filter: string, flat array, or nested array for OR-group clusters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum KeywordFilter {
    /// Single keyword
    Single(String),
    /// Flat array (AND within group)
    Multiple(Vec<String>),
    /// Nested arrays (OR across groups)
    Nested(Vec<Vec<String>>),
}

/// Base fields shared by all tools that produce output.
/// Each tool embeds this as a single field instead of repeating these params.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutputOptions {
    /// Output format: "toon" (default) or "json"
    pub format: Option<String>,
}

/// Base fields shared by all news/source discovery tools.
/// Covers filtering, date ranges, geographic scoping, and content matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveryFilters {
    /// Pool IDs (e.g. ["GLOBAL_TECH_CYBER"])
    pub pools: Option<Vec<String>>,
    /// Source IDs (overrides pools)
    pub sources: Option<Vec<String>>,
    /// ISO country codes (e.g. ["US", "IN"])
    pub countries: Option<Vec<String>>,
    /// City names filter
    pub cities: Option<Vec<String>>,
    /// Domain filter
    pub domains: Option<Vec<String>>,
    /// Start date (ISO 8601)
    pub start: Option<String>,
    /// End date (ISO 8601)
    pub end: Option<String>,
    /// Keywords (string, array, or nested arrays)
    pub keywords: Option<KeywordFilter>,
    /// Exclude keywords
    pub exclude_keywords: Option<Vec<String>>,
    /// AND logic (default: OR)
    pub match_all: Option<bool>,
    /// Max items (1-500, default: 20)
    pub limit: Option<i32>,
    /// Cache mode: fresh/all/only
    pub cache_mode: Option<String>,
}

/// Base fields shared by crawl/depth-aware tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DepthOptions {
    /// Crawl depth: shallow/medium/deep
    pub depth: Option<String>,
}

/// Format helper: extract format from OutputOptions or return default "toon".
pub fn resolve_format(opts: &OutputOptions) -> String {
    opts.format.as_deref().unwrap_or("toon").to_string()
}

/// Format helper: resolve format from an Option<String> directly.
pub fn resolve_format_opt(format: &Option<String>) -> String {
    format.as_deref().unwrap_or("toon").to_string()
}
