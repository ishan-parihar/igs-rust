//! Entity resolution and normalization.
//!
//! Provides:
//! - Entity normalization (lowercase, strip punctuation, canonical forms)
//! - Alias detection (abbreviations, common variations)
//! - Wikidata Q-ID linking (optional, via SPARQL API)
//! - Entity type classification (Person, Organization, GPE, etc.)
//!
//! The normalization is offline and deterministic. Wikidata linking is
//! optional and requires network access.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedEntity {
    pub original_name: String,
    pub canonical_name: String,
    pub entity_type: String,
    pub normalized_id: String,
    pub aliases: Vec<String>,
    pub wikidata_id: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolutionOutput {
    pub entities: Vec<ResolvedEntity>,
    pub count: usize,
}

/// Common entity aliases for normalization (offline lookup).
/// This is a small seed dictionary — can be extended or loaded from a file.
fn alias_dictionary() -> HashMap<String, (String, String)> {
    // Maps lowercase alias → (canonical_name, entity_type)
    let mut dict: HashMap<String, (String, String)> = HashMap::new();

    // Organizations
    dict.insert("openai".into(), ("OpenAI".into(), "Organization".into()));
    dict.insert("oai".into(), ("OpenAI".into(), "Organization".into()));
    dict.insert("google".into(), ("Google".into(), "Organization".into()));
    dict.insert("alphabet".into(), ("Alphabet Inc.".into(), "Organization".into()));
    dict.insert("microsoft".into(), ("Microsoft".into(), "Organization".into()));
    dict.insert("msft".into(), ("Microsoft".into(), "Organization".into()));
    dict.insert("apple".into(), ("Apple Inc.".into(), "Organization".into()));
    dict.insert("aapl".into(), ("Apple Inc.".into(), "Organization".into()));
    dict.insert("amazon".into(), ("Amazon".into(), "Organization".into()));
    dict.insert("amzn".into(), ("Amazon".into(), "Organization".into()));
    dict.insert("meta".into(), ("Meta Platforms".into(), "Organization".into()));
    dict.insert("facebook".into(), ("Meta Platforms".into(), "Organization".into()));
    dict.insert("nvidia".into(), ("NVIDIA".into(), "Organization".into()));
    dict.insert("nvda".into(), ("NVIDIA".into(), "Organization".into()));
    dict.insert("tesla".into(), ("Tesla, Inc.".into(), "Organization".into()));
    dict.insert("tsla".into(), ("Tesla, Inc.".into(), "Organization".into()));

    // Countries (GPE - Geo-Political Entity)
    dict.insert("usa".into(), ("United States".into(), "GPE".into()));
    dict.insert("us".into(), ("United States".into(), "GPE".into()));
    dict.insert("u.s.".into(), ("United States".into(), "GPE".into()));
    dict.insert("u.s.a.".into(), ("United States".into(), "GPE".into()));
    dict.insert("uk".into(), ("United Kingdom".into(), "GPE".into()));
    dict.insert("u.k.".into(), ("United Kingdom".into(), "GPE".into()));
    dict.insert("britain".into(), ("United Kingdom".into(), "GPE".into()));
    dict.insert("eu".into(), ("European Union".into(), "GPE".into()));
    dict.insert("e.u.".into(), ("European Union".into(), "GPE".into()));
    dict.insert("russia".into(), ("Russia".into(), "GPE".into()));
    dict.insert("china".into(), ("China".into(), "GPE".into()));
    dict.insert("prc".into(), ("China".into(), "GPE".into()));
    dict.insert("india".into(), ("India".into(), "GPE".into()));
    dict.insert("israel".into(), ("Israel".into(), "GPE".into()));
    dict.insert("iran".into(), ("Iran".into(), "GPE".into()));
    dict.insert("north korea".into(), ("North Korea".into(), "GPE".into()));
    dict.insert("dprk".into(), ("North Korea".into(), "GPE".into()));
    dict.insert("south korea".into(), ("South Korea".into(), "GPE".into()));
    dict.insert("rok".into(), ("South Korea".into(), "GPE".into()));

    // Organizations (government/military)
    dict.insert("nato".into(), ("NATO".into(), "Organization".into()));
    dict.insert("un".into(), ("United Nations".into(), "Organization".into()));
    dict.insert("u.n.".into(), ("United Nations".into(), "Organization".into()));
    dict.insert("cia".into(), ("CIA".into(), "Organization".into()));
    dict.insert("fbi".into(), ("FBI".into(), "Organization".into()));
    dict.insert("nsa".into(), ("NSA".into(), "Organization".into()));
    dict.insert("doj".into(), ("Department of Justice".into(), "Organization".into()));
    dict.insert("dod".into(), ("Department of Defense".into(), "Organization".into()));
    dict.insert("epa".into(), ("EPA".into(), "Organization".into()));
    dict.insert("fda".into(), ("FDA".into(), "Organization".into()));
    dict.insert("cdc".into(), ("CDC".into(), "Organization".into()));
    dict.insert("who".into(), ("WHO".into(), "Organization".into()));
    dict.insert("w.h.o.".into(), ("WHO".into(), "Organization".into()));

    // Tech terms
    dict.insert("ai".into(), ("Artificial Intelligence".into(), "Concept".into()));
    dict.insert("a.i.".into(), ("Artificial Intelligence".into(), "Concept".into()));
    dict.insert("ml".into(), ("Machine Learning".into(), "Concept".into()));
    dict.insert("nlp".into(), ("Natural Language Processing".into(), "Concept".into()));
    dict.insert("llm".into(), ("Large Language Model".into(), "Concept".into()));

    dict
}

/// Normalize an entity name: lowercase, strip punctuation, trim whitespace.
pub fn normalize_entity_name(name: &str) -> String {
    let normalized: String = name
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Generate a deterministic normalized ID from an entity name.
/// This is used as the `normalized_id` field on `EntityInfo`.
pub fn generate_normalized_id(name: &str) -> String {
    let normalized = normalize_entity_name(name);
    if normalized.is_empty() {
        return String::new();
    }
    // Use a simple hash: first 12 chars of SHA256-like hex
    let mut hash: u64 = 5381;
    for byte in normalized.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    format!("ent_{:012x}", hash)
}

/// Resolve a list of entity names to their canonical forms.
/// Uses the offline alias dictionary for known entities.
/// Unknown entities are normalized but not linked to a canonical form.
/// Entities that resolve to the same canonical name are deduplicated.
pub fn resolve_entities(names: &[String]) -> EntityResolutionOutput {
    let dict = alias_dictionary();
    let mut seen: HashMap<String, ResolvedEntity> = HashMap::new();

    for name in names {
        let normalized = normalize_entity_name(name);
        if normalized.is_empty() {
            continue;
        }

        // Check alias dictionary
        let (canonical_name, entity_type, aliases, confidence) =
            if let Some((canonical, etype)) = dict.get(&normalized) {
                // Found in alias dict — high confidence
                (canonical.clone(), etype.clone(), vec![normalized.clone()], 0.95)
            } else {
                // Unknown entity — use normalized name as canonical, lower confidence
                let etype = classify_entity_type(name);
                (title_case(&normalized), etype, vec![], 0.5)
            };

        // Dedup by canonical name (not normalized name) so aliases merge
        let dedup_key = canonical_name.to_lowercase();
        if seen.contains_key(&dedup_key) {
            // Already have this canonical entity — just add the alias if not present
            if let Some(existing) = seen.get_mut(&dedup_key) {
                if !existing.aliases.contains(&normalized) {
                    existing.aliases.push(normalized);
                }
            }
            continue;
        }

        let resolved = ResolvedEntity {
            original_name: name.clone(),
            canonical_name: canonical_name.clone(),
            entity_type,
            normalized_id: generate_normalized_id(&normalized),
            aliases,
            wikidata_id: None,
            confidence,
        };

        seen.insert(dedup_key, resolved);
    }

    let mut entities: Vec<ResolvedEntity> = seen.into_values().collect();
    entities.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    let count = entities.len();

    EntityResolutionOutput { entities, count }
}

/// Classify entity type using simple heuristics.
fn classify_entity_type(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.is_empty() {
        return "Unknown".into();
    }

    // Multi-word capitalized → likely Person or Organization
    if words.len() > 1 {
        let first = words[0];
        // Common organization indicators
        if name.contains("Inc") || name.contains("Corp") || name.contains("Ltd")
            || name.contains("LLC") || name.contains("Company")
        {
            return "Organization".into();
        }
        // Common GPE indicators
        if name.contains("Republic") || name.contains("Kingdom") || name.contains("States") {
            return "GPE".into();
        }
        // Default multi-word capitalized → Person
        if first.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            return "Person".into();
        }
    }

    // Single word — check if it's an acronym (all caps)
    if name.chars().all(|c| c.is_uppercase()) && name.len() >= 2 {
        return "Organization".into();
    }

    "Unknown".into()
}

/// Convert a normalized (lowercase) string to title case.
fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Look up a Wikidata Q-ID for an entity name via the SPARQL API.
/// This is an optional network call — returns None on failure.
pub async fn lookup_wikidata_id(name: &str) -> Option<String> {
    let url = format!(
        "https://www.wikidata.org/w/api.php?action=wbsearchentities&search={}&language=en&format=json&limit=1",
        url::form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>()
    );

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json["search"][0]["id"].as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_entity_name() {
        assert_eq!(normalize_entity_name("  OpenAI, Inc.  "), "openai inc");
        // Dots are stripped, letters kept together: "U.S.A." → "usa"
        assert_eq!(normalize_entity_name("U.S.A."), "usa");
        assert_eq!(normalize_entity_name("North Korea"), "north korea");
    }

    #[test]
    fn test_generate_normalized_id_deterministic() {
        let id1 = generate_normalized_id("OpenAI");
        let id2 = generate_normalized_id("OpenAI");
        let id3 = generate_normalized_id("Google");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert!(id1.starts_with("ent_"));
    }

    #[test]
    fn test_resolve_known_entity() {
        let result = resolve_entities(&["OpenAI".into(), "OAI".into()]);
        // Both should resolve to the same canonical name
        assert_eq!(result.count, 1);
        assert_eq!(result.entities[0].canonical_name, "OpenAI");
        assert_eq!(result.entities[0].entity_type, "Organization");
        assert!(result.entities[0].confidence > 0.9);
    }

    #[test]
    fn test_resolve_unknown_entity() {
        let result = resolve_entities(&["Some Unknown Person".into()]);
        assert_eq!(result.count, 1);
        assert_eq!(result.entities[0].confidence, 0.5);
    }

    #[test]
    fn test_resolve_multiple_entities() {
        let result = resolve_entities(&[
            "USA".into(),
            "U.S.".into(),
            "Google".into(),
            "Unknown Entity".into(),
        ]);
        // USA and U.S. resolve to same canonical (United States), so 3 unique
        assert_eq!(result.count, 3);
    }

    #[test]
    fn test_classify_entity_type() {
        assert_eq!(classify_entity_type("John Smith"), "Person");
        assert_eq!(classify_entity_type("Acme Corp"), "Organization");
        assert_eq!(classify_entity_type("United Kingdom"), "GPE");
        assert_eq!(classify_entity_type("NASA"), "Organization");
    }

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("hello world"), "Hello World");
        assert_eq!(title_case("openai"), "Openai");
    }

    #[test]
    fn test_alias_dictionary_coverage() {
        let dict = alias_dictionary();
        assert!(dict.contains_key("openai"));
        assert!(dict.contains_key("usa"));
        assert!(dict.contains_key("nato"));
        assert!(dict.contains_key("ai"));
    }
}
