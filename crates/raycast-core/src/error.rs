//! The error algebra for the domain layer.
//!
//! Errors are tagged by *cause*, not by message. Every variant carries the
//! information a caller needs to decide whether to retry, to fall back, or to
//! surface a message to the user. Recovery guidance is part of the type, not
//! something reconstructed later from a string.

use std::fmt;

/// Result alias used across the domain layer.
pub type CoreResult<T> = Result<T, CoreError>;

/// Everything that can go wrong inside the domain layer.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// An item was constructed without the fields its kind requires.
    #[error("invalid item: {0}")]
    InvalidItem(String),

    /// A command could not be resolved.
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    /// The caller supplied an input that cannot be interpreted.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A query produced no usable result.
    #[error("no match for query")]
    NoMatch,

    /// A feature exists in the model but this build does not implement it.
    ///
    /// This is deliberately distinct from `InvalidInput`: the call is
    /// well-formed, the capability is simply absent. Keeping the two apart
    /// stops "not built yet" from being reported as "you typed it wrong".
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Underlying I/O failed. The source error is retained for the cause.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A string was not valid UTF-8 where UTF-8 was required.
    #[error("invalid utf-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

impl CoreError {
    /// Whether retrying the same call could plausibly succeed.
    ///
    /// Only genuinely transient failures qualify. A malformed query or an
    /// absent capability will fail identically forever.
    pub fn is_retryable(&self) -> bool {
        matches!(self, CoreError::Io(_))
    }

    /// A short, user-facing remediation line.
    ///
    /// Returns `None` when the error is self-explanatory and needs no advice.
    pub fn remediation(&self) -> Option<String> {
        match self {
            CoreError::UnknownCommand(name) => {
                Some(format!("no command is registered under `{name}`"))
            }
            CoreError::Unsupported(what) => {
                Some(format!("`{what}` is not available in this build"))
            }
            CoreError::Io(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                Some("the file or device is not accessible with the current permissions".into())
            }
            CoreError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Some("the requested path no longer exists".into())
            }
            _ => None,
        }
    }
}

/// A domain error paired with the remediation a caller should show.
#[derive(Debug)]
pub struct ErrorReport {
    pub error: CoreError,
    pub remediation: Option<String>,
}

impl fmt::Display for ErrorReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.remediation {
            Some(advice) => write!(f, "{} ({advice})", self.error),
            None => write!(f, "{}", self.error),
        }
    }
}

impl From<CoreError> for ErrorReport {
    fn from(error: CoreError) -> Self {
        let remediation = error.remediation();
        Self { error, remediation }
    }
}
