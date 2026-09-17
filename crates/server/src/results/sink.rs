use std::{error::Error, fmt, future::Future};

use healthcheck_core::CheckResult;

/// Receives completed check results.
pub trait ResultSink {
    fn record(
        &self,
        result: CheckResult,
    ) -> impl Future<Output = Result<(), ResultSinkError>> + Send + '_;
}

/// Error returned by result sinks when recording fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultSinkError {
    message: String,
}

impl ResultSinkError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ResultSinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ResultSinkError {}
