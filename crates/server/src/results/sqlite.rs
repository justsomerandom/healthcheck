use healthcheck_core::CheckResult;

use crate::persistence::CheckResultRepository;

use super::{ResultSink, ResultSinkError};

/// Result sink that records completed check results in SQLite.
#[derive(Debug, Clone)]
pub struct SqliteResultSink {
    repository: CheckResultRepository,
}

impl SqliteResultSink {
    pub fn new(repository: CheckResultRepository) -> Self {
        Self { repository }
    }
}

impl ResultSink for SqliteResultSink {
    async fn record(&self, result: CheckResult) -> Result<(), ResultSinkError> {
        self.repository
            .insert(&result)
            .await
            .map_err(|error| ResultSinkError::new(error.to_string()))
    }
}
