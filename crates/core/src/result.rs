use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::HealthStatus;

/// Result of one completed check execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CheckResult {
    pub id: Uuid,
    pub check_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub status: HealthStatus,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl CheckResult {
    /// Creates a check result with a generated id and completion timestamp.
    pub fn new(
        check_id: Uuid,
        status: HealthStatus,
        duration_ms: u64,
        error: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            check_id,
            checked_at: Utc::now(),
            status,
            duration_ms,
            error,
        }
    }

    /// Reconstructs a check result from previously persisted domain fields.
    pub fn from_persisted(
        id: Uuid,
        check_id: Uuid,
        checked_at: DateTime<Utc>,
        status: HealthStatus,
        duration_ms: u64,
        error: Option<String>,
    ) -> Self {
        Self {
            id,
            check_id,
            checked_at,
            status,
            duration_ms,
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_check_result() {
        let check_id = Uuid::new_v4();
        let result = CheckResult::new(
            check_id,
            HealthStatus::Unhealthy,
            250,
            Some("connection refused".to_owned()),
        );

        assert_eq!(result.check_id, check_id);
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(result.duration_ms, 250);
        assert_eq!(result.error.as_deref(), Some("connection refused"));
        assert_ne!(result.id, Uuid::nil());
    }

    #[test]
    fn reconstructs_persisted_check_result() {
        let id = Uuid::new_v4();
        let check_id = Uuid::new_v4();
        let checked_at = Utc::now();
        let result =
            CheckResult::from_persisted(id, check_id, checked_at, HealthStatus::Healthy, 12, None);

        assert_eq!(result.id, id);
        assert_eq!(result.check_id, check_id);
        assert_eq!(result.checked_at, checked_at);
    }
}
