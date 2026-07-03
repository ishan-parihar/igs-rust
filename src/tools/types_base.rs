//! Base input types for shared fields across IGS tool categories.
//! Eliminates ~300 tokens of duplicated `format: Option<String>` across 15 structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum KeywordFilter {
    /// Single keyword
    Single(String),
    /// Flat array (AND)
    Multiple(Vec<String>),
    /// Nested arrays (OR)
    Nested(Vec<Vec<String>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutputOptions {
    /// Output format
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveryFilters {
    /// Pool IDs
    pub pools: Option<Vec<String>>,
    /// Source IDs (overrides pools)
    pub sources: Option<Vec<String>>,
    /// Country codes
    pub countries: Option<Vec<String>>,
    /// City names filter
    pub cities: Option<Vec<String>>,
    /// Domain filter
    pub domains: Option<Vec<String>>,
    /// Start date
    pub start: Option<String>,
    /// End date
    pub end: Option<String>,
    /// Keywords
    pub keywords: Option<KeywordFilter>,
    /// Exclude keywords
    pub exclude_keywords: Option<Vec<String>>,
    /// AND logic (default: OR)
    pub match_all: Option<bool>,
    /// Max items (1-500)
    pub limit: Option<i32>,
    /// Cache mode: fresh|all|only
    pub cache_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DepthOptions {
    /// Crawl depth
    pub depth: Option<String>,
}
