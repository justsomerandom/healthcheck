use thiserror::Error;

/// Validation errors raised while constructing domain values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DomainError {
    /// A project or service name was empty after trimming whitespace.
    #[error("{entity} name must not be empty")]
    EmptyName { entity: &'static str },

    /// An HTTP check URL was empty after trimming whitespace.
    #[error("http check URL must not be empty")]
    EmptyHttpUrl,

    /// Check intervals must be positive.
    #[error("check interval must be greater than zero")]
    ZeroInterval,

    /// Check timeouts must be positive.
    #[error("check timeout must be greater than zero")]
    ZeroTimeout,

    /// HTTP status codes are limited to the standard 100-599 range.
    #[error("expected HTTP status must be in the range 100..=599, got {status}")]
    InvalidHttpStatus { status: u16 },
}
