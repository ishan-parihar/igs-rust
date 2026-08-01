//! TOON Helper — Shared output formatting for AXI-compliant CLIs
//!
//! Provides token-efficient TOON output (AXI §1) and format switching.

use serde::Serialize;

/// TOON-format encode a serializable value for AI-agent token-efficiency
///
/// Falls back to JSON if TOON encoding fails.
pub fn toon_encode<T: Serialize>(value: &T) -> String {
    toon_format::encode_default(value)
        .unwrap_or_else(|_| serde_json::to_string(value).unwrap_or_default())
}

/// Serialize a value to the requested format ("toon" or "json") as a String.
///
/// Default format is TOON for ~40% token savings (AXI §1).
pub fn format_text<T: Serialize>(value: &T, format: &str) -> String {
    if format == "json" {
        serde_json::to_string_pretty(value).unwrap_or_default()
    } else {
        toon_encode(value)
    }
}

/// Truncate a string to max characters with ellipsis.
///
/// Used for AXI §3 content truncation in detail views.
pub fn truncate_str(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!(
            "{}...\n  ... (truncated, {} chars total)",
            truncated, char_count
        )
    }
}

/// AXI §3: Recursively truncate long string fields in a JSON value.
///
/// Fields exceeding `max_chars` are truncated with a total-length indicator.
/// Used by both slideforge-rust and social-forge at the output boundary.
pub fn truncate_json_strings(value: &serde_json::Value, max_chars: usize) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), truncate_json_strings(v, max_chars));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| truncate_json_strings(v, max_chars))
                .collect(),
        ),
        serde_json::Value::String(s) if s.len() > max_chars => {
            let total = s.len();
            let truncated: String = s.chars().take(max_chars).collect();
            serde_json::json!(format!(
                "{}... (truncated, {} chars total)",
                truncated, total
            ))
        }
        other => other.clone(),
    }
}

/// Print output to stdout in the requested format.
///
/// Combined helper for the common pattern of format → println!.
pub fn print_output<T: Serialize>(format: &str, value: &T) {
    let text = format_text(value, format);
    println!("{}", text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toon_encode_simple() {
        let value = serde_json::json!({"name": "Alice", "age": 30});
        let result = toon_encode(&value);
        assert!(!result.is_empty());
        assert!(!result.starts_with('{'));
    }

    #[test]
    fn test_toon_encode_array() {
        let value = serde_json::json!([1, 2, 3]);
        let result = toon_encode(&value);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_format_text_json() {
        let value = serde_json::json!({"key": "value"});
        let result = format_text(&value, "json");
        assert!(result.starts_with('{'));
    }

    #[test]
    fn test_format_text_toon() {
        let value = serde_json::json!({"key": "value"});
        let result = format_text(&value, "toon");
        assert!(!result.starts_with('{'));
    }

    #[test]
    fn test_truncate_str_short() {
        let result = truncate_str("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        let result = truncate_str("hello world this is a long string", 10);
        assert!(result.contains("..."));
        assert!(result.contains("chars total"));
    }

    #[test]
    fn test_truncate_json_strings_short() {
        let val = serde_json::json!({"name": "hello", "count": 42});
        let result = truncate_json_strings(&val, 100);
        assert_eq!(result, val);
    }

    #[test]
    fn test_truncate_json_strings_long() {
        let long_text = "a".repeat(600);
        let val = serde_json::json!({"body": long_text, "title": "short"});
        let result = truncate_json_strings(&val, 500);
        let body = result["body"].as_str().unwrap();
        assert!(body.contains("truncated"));
        assert!(body.contains("600 chars total"));
        assert_eq!(result["title"], "short");
    }

    #[test]
    fn test_truncate_json_strings_nested() {
        let long_text = "b".repeat(700);
        let val = serde_json::json!({"nested": {"body": long_text}, "other": ["c".repeat(600)]});
        let result = truncate_json_strings(&val, 500);
        assert!(result["nested"]["body"]
            .as_str()
            .unwrap()
            .contains("truncated"));
        assert!(result["other"][0].as_str().unwrap().contains("truncated"));
    }
}

#[cfg(test)]
mod output_tests {
    use super::*;

    #[test]
    fn test_print_output_toon() {
        let value = serde_json::json!({"name": "test", "count": 42});
        // print_output writes to stdout, so we just verify it doesn't panic
        print_output("toon", &value);
    }

    #[test]
    fn test_print_output_json() {
        let value = serde_json::json!({"name": "test", "count": 42});
        // print_output writes to stdout, so we just verify it doesn't panic
        print_output("json", &value);
    }
}
