use std::fmt;

/// Unified error type for IGS internal operations.
///
/// Server tool handlers remain `Result<T, String>` at the rmcp boundary,
/// but internal tool functions use `AppError` for structured error handling.
/// The `From<AppError> for String` impl enables `?` in handlers.
#[derive(Debug)]
pub enum AppError {
    /// Configuration / settings / pool / source file errors.
    Config(String),
    /// HTTP client or network errors.
    Http(reqwest::Error),
    /// JSON serialization / deserialization errors.
    Json(serde_json::Error),
    /// File I/O errors.
    Io(std::io::Error),
    /// Feed parsing errors.
    Feed(String),
    /// Input validation errors.
    Validation(String),
    /// Generic catch-all (keeps migration simple for edge cases).
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Config(e) => write!(f, "Configuration error: {e}"),
            AppError::Http(e) => write!(f, "HTTP error: {e}"),
            AppError::Json(e) => write!(f, "JSON error: {e}"),
            AppError::Io(e) => write!(f, "IO error: {e}"),
            AppError::Feed(e) => write!(f, "Feed parsing error: {e}"),
            AppError::Validation(e) => write!(f, "Validation error: {e}"),
            AppError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Http(e) => Some(e),
            AppError::Json(e) => Some(e),
            AppError::Io(e) => Some(e),
            _ => None,
        }
    }
}

// ─── From impls for ergonomic ? operator ──────────────────────

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Json(e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

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
