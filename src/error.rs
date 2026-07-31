/// Unified error type for IGS internal operations.
///
/// Server tool handlers remain `Result<T, String>` at the rmcp boundary,
/// but internal tool functions use `AppError` for structured error handling.
/// The `From<AppError> for String` impl enables `?` in handlers.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Configuration / settings / pool / source file errors.
    #[error("Configuration error: {0}")]
    Config(String),

    /// HTTP client or network errors.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization / deserialization errors.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// File I/O errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Feed parsing errors.
    #[error("Feed parsing error: {0}")]
    Feed(String),

    /// Input validation errors.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Generic catch-all (keeps migration simple for edge cases).
    #[error("{0}")]
    Other(String),
}

// ─── Boundary From impls (not auto-derived by thiserror) ──────

/// Convert AppError → String for rmcp boundary handlers.
/// Enables `?` in `fn handler() -> Result<CallToolResult, String>`.
impl From<AppError> for String {
    fn from(e: AppError) -> String {
        e.to_string()
    }
}

/// Convert String → AppError for existing `.map_err(|e| format!(...))?` patterns.
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Other(s)
    }
}

/// Convert &str → AppError for existing `Err("...")` and `ok_or("...")` patterns.
impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Other(s.to_string())
    }
}

// ─── Convenience constructors ─────────────────────────────────

impl AppError {
    pub fn config(msg: impl Into<String>) -> Self {
        AppError::Config(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        AppError::Validation(msg.into())
    }

    pub fn feed(msg: impl Into<String>) -> Self {
        AppError::Feed(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        AppError::Other(msg.into())
    }
}

/// Type alias for convenience.
pub type AppResult<T> = Result<T, AppError>;
