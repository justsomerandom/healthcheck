use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::DomainError;

/// Configured health check for a monitored service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Check {
    pub id: Uuid,
    pub service_id: Uuid,
    pub kind: CheckKind,
    pub interval_seconds: u64,
    pub timeout_ms: u64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl Check {
    /// Creates an enabled check with a generated id and creation timestamp.
    pub fn new(
        service_id: Uuid,
        kind: CheckKind,
        interval_seconds: u64,
        timeout_ms: u64,
    ) -> Result<Self, DomainError> {
        if interval_seconds == 0 {
            return Err(DomainError::ZeroInterval);
        }

        if timeout_ms == 0 {
            return Err(DomainError::ZeroTimeout);
        }

        Ok(Self {
            id: Uuid::new_v4(),
            service_id,
            kind,
            interval_seconds,
            timeout_ms,
            enabled: true,
            created_at: Utc::now(),
        })
    }

    /// Reconstructs a check from previously persisted domain fields.
    pub fn from_persisted(
        id: Uuid,
        service_id: Uuid,
        kind: CheckKind,
        interval_seconds: u64,
        timeout_ms: u64,
        enabled: bool,
        created_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        if interval_seconds == 0 {
            return Err(DomainError::ZeroInterval);
        }

        if timeout_ms == 0 {
            return Err(DomainError::ZeroTimeout);
        }

        Ok(Self {
            id,
            service_id,
            kind,
            interval_seconds,
            timeout_ms,
            enabled,
            created_at,
        })
    }
}

/// Type-specific configuration for a health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CheckKind {
    /// HTTP status check against a configured URL.
    #[non_exhaustive]
    Http { url: String, expected_status: u16 },
}

impl CheckKind {
    /// Creates an HTTP check kind with light domain validation.
    pub fn http(url: impl Into<String>, expected_status: u16) -> Result<Self, DomainError> {
        let url = url.into().trim().to_owned();

        if url.is_empty() {
            return Err(DomainError::EmptyHttpUrl);
        }

        if !(100..=599).contains(&expected_status) {
            return Err(DomainError::InvalidHttpStatus {
                status: expected_status,
            });
        }

        Ok(Self::Http {
            url,
            expected_status,
        })
    }
}

/// Interpreted health state for a service or check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[test]
    fn creates_valid_http_check() {
        let service_id = Uuid::new_v4();
        let kind = CheckKind::http("https://example.com/health", 200).expect("kind is valid");
        let check = Check::new(service_id, kind.clone(), 30, 1_000).expect("check is valid");

        assert_eq!(check.service_id, service_id);
        assert_eq!(check.kind, kind);
        assert_eq!(check.interval_seconds, 30);
        assert_eq!(check.timeout_ms, 1_000);
        assert!(check.enabled);
        assert_ne!(check.id, Uuid::nil());
    }

    #[test]
    fn rejects_zero_interval() {
        let kind = CheckKind::http("https://example.com/health", 200).expect("kind is valid");
        let error =
            Check::new(Uuid::new_v4(), kind, 0, 1_000).expect_err("zero interval should fail");

        assert_eq!(error, DomainError::ZeroInterval);
    }

    #[test]
    fn rejects_zero_timeout() {
        let kind = CheckKind::http("https://example.com/health", 200).expect("kind is valid");
        let error = Check::new(Uuid::new_v4(), kind, 30, 0).expect_err("zero timeout should fail");

        assert_eq!(error, DomainError::ZeroTimeout);
    }

    #[test]
    fn accepts_valid_expected_http_status() {
        let kind = CheckKind::http("https://example.com/health", 599).expect("status is valid");

        assert_eq!(
            kind,
            CheckKind::Http {
                url: "https://example.com/health".to_owned(),
                expected_status: 599,
            }
        );
    }

    #[test]
    fn rejects_invalid_expected_http_status() {
        let error = CheckKind::http("https://example.com/health", 99)
            .expect_err("status below 100 should fail");

        assert_eq!(error, DomainError::InvalidHttpStatus { status: 99 });
    }

    #[test]
    fn rejects_empty_http_url() {
        let error = CheckKind::http("  ", 200).expect_err("blank URL should fail");

        assert_eq!(error, DomainError::EmptyHttpUrl);
    }

    #[test]
    fn reconstructs_persisted_check() {
        let id = Uuid::new_v4();
        let service_id = Uuid::new_v4();
        let created_at = Utc::now();
        let kind = CheckKind::http("https://example.com/health", 204).expect("kind is valid");
        let check = Check::from_persisted(id, service_id, kind.clone(), 60, 500, false, created_at)
            .expect("check is valid");

        assert_eq!(check.id, id);
        assert_eq!(check.service_id, service_id);
        assert_eq!(check.kind, kind);
        assert_eq!(check.interval_seconds, 60);
        assert_eq!(check.timeout_ms, 500);
        assert!(!check.enabled);
        assert_eq!(check.created_at, created_at);
    }

    #[test]
    fn health_status_supports_equality_and_serde() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
        assert_serde::<HealthStatus>();
    }

    fn assert_serde<T>()
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
    }
}
