use thiserror::Error;

/// Errors raised by persistence and SQL/domain mapping code.
#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("invalid UUID in {field}: {value}")]
    InvalidUuid {
        field: &'static str,
        value: String,
        source: uuid::Error,
    },

    #[error("invalid UTC timestamp in {field}: {value}")]
    InvalidTimestamp {
        field: &'static str,
        value: String,
        source: chrono::ParseError,
    },

    #[error("invalid persisted enum value for {kind}: {value}")]
    InvalidEnum { kind: &'static str, value: String },

    #[error("persisted numeric value out of range for {field}: {value}")]
    NumericOutOfRange { field: &'static str, value: i64 },

    #[error("invalid persisted domain data: {0}")]
    InvalidDomain(#[from] healthcheck_core::DomainError),
}
