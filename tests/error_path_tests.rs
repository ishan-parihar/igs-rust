//! Error-path tests: verify AppError types, Display, and key validation errors.
//!
//! These complement the happy-path platform_integration_test.rs.

use igs_rust_mcp::error::{AppError, AppResult};
use std::error::Error;

// ─── AppError Display ────────────────────────────────────────

#[test]
fn apperror_display_config() {
    let e = AppError::Config("missing pool".into());
    assert_eq!(e.to_string(), "Configuration error: missing pool");
}

#[test]
fn apperror_display_validation() {
    let e = AppError::Validation("query is empty".into());
    assert_eq!(e.to_string(), "Validation error: query is empty");
}

#[test]
fn apperror_display_feed() {
    let e = AppError::Feed("invalid RSS".into());
    assert_eq!(e.to_string(), "Feed parsing error: invalid RSS");
}

#[test]
fn apperror_display_other() {
    let e = AppError::Other("something broke".into());
    assert_eq!(e.to_string(), "something broke");
}

#[test]
fn apperror_display_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let e = AppError::Io(io_err);
    assert!(e.to_string().contains("IO error"));
    assert!(e.to_string().contains("not found"));
}

// ─── From impls ──────────────────────────────────────────────

#[test]
fn from_string_into_apperror() {
    let e: AppError = "bad input".to_string().into();
    assert!(matches!(e, AppError::Other(_)));
    assert_eq!(e.to_string(), "bad input");
}

#[test]
fn from_str_into_apperror() {
    let e: AppError = "bad input".into();
    assert!(matches!(e, AppError::Other(_)));
    assert_eq!(e.to_string(), "bad input");
}

#[test]
fn apperror_into_string() {
    let e = AppError::Config("test".into());
    let s: String = e.into();
    assert_eq!(s, "Configuration error: test");
}

#[test]
fn io_error_into_apperror() {
    let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
    let e: AppError = io_err.into();
    assert!(matches!(e, AppError::Io(_)));
}

#[test]
fn json_error_into_apperror() {
    let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let e: AppError = json_err.into();
    assert!(matches!(e, AppError::Json(_)));
}

// ─── AppResult propagation ───────────────────────────────────

#[test]
fn appresult_question_mark_propagates() {
    let result: AppResult<serde_json::Value> =
        serde_json::from_str("invalid").map_err(AppError::from);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::Json(_)));
}

#[test]
fn appresult_question_mark_io() {
    let result: AppResult<String> =
        std::fs::read_to_string("/nonexistent/path/that/does/not/exist.json")
            .map_err(AppError::from);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), AppError::Io(_)));
}

// ─── Convenience constructors ────────────────────────────────

#[test]
fn apperror_config_constructor() {
    let e = AppError::config("no settings file");
    assert!(matches!(e, AppError::Config(_)));
    assert!(e.to_string().contains("no settings file"));
}

#[test]
fn apperror_validation_constructor() {
    let e = AppError::validation("empty query");
    assert!(matches!(e, AppError::Validation(_)));
}

#[test]
fn apperror_feed_constructor() {
    let e = AppError::feed("bad xml");
    assert!(matches!(e, AppError::Feed(_)));
}

#[test]
fn apperror_other_constructor() {
    let e = AppError::other("fallback");
    assert!(matches!(e, AppError::Other(_)));
}

// ─── Source chain ────────────────────────────────────────────

#[test]
fn apperror_source_returns_inner_for_io() {
    let io_err = std::io::Error::other("disk full");
    let e = AppError::Io(io_err);
    assert!(e.source().is_some());
}

#[test]
fn apperror_source_returns_none_for_config() {
    let e = AppError::Config("test".into());
    assert!(e.source().is_none());
}

// ─── Functional error paths (validation) ─────────────────────

#[tokio::test]
async fn youtube_search_empty_query_fails() {
    use igs_rust_mcp::tools::types::*;
    use igs_rust_mcp::tools::youtube;

    let input = YoutubeSearchInput {
        query: "".to_string(),
        limit: Some(5),
    };
    let result = youtube::youtube_search(input).await;
    assert!(result.is_err(), "Empty query should return an error");
}

#[tokio::test]
async fn reddit_search_empty_query_fails() {
    use igs_rust_mcp::tools::reddit;
    use igs_rust_mcp::tools::types::*;
    use igs_rust_mcp::tools::types_base::OutputOptions;

    let input = RedditSearchInput {
        query: "".to_string(),
        subreddits: None,
        sort: None,
        time: None,
        limit: Some(5),
        output: OutputOptions { format: None },
    };
    let result = reddit::reddit_search(input).await;
    assert!(result.is_err(), "Empty query should return an error");
}

// ─── Standard library error roundtrip ────────────────────────

#[test]
fn apperror_display_is_human_readable() {
    let errors = vec![
        AppError::Config("pool not found".into()),
        AppError::Validation("missing required field".into()),
        AppError::Feed("malformed atom feed".into()),
        AppError::Other("upstream failure".into()),
    ];
    for e in errors {
        let s = e.to_string();
        assert!(!s.is_empty(), "Display should produce non-empty string");
        // Should not contain Rust debug formatting artifacts
        assert!(
            !s.contains("AppError"),
            "Display should not leak enum name: {s}"
        );
    }
}
