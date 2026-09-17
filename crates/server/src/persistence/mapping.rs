use chrono::{DateTime, SecondsFormat, Utc};
use healthcheck_core::{Check, CheckKind, CheckResult, HealthStatus, Project, Service};
use sqlx::{sqlite::SqliteRow, Row};
use uuid::Uuid;

use super::PersistenceError;

pub(super) fn uuid_to_db(uuid: Uuid) -> String {
    uuid.to_string()
}

pub(super) fn uuid_from_db(field: &'static str, value: String) -> Result<Uuid, PersistenceError> {
    value
        .parse()
        .map_err(|source| PersistenceError::InvalidUuid {
            field,
            value,
            source,
        })
}

pub(super) fn timestamp_to_db(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

pub(super) fn timestamp_from_db(
    field: &'static str,
    value: String,
) -> Result<DateTime<Utc>, PersistenceError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|source| PersistenceError::InvalidTimestamp {
            field,
            value,
            source,
        })
}

pub(super) fn u64_to_db(value: u64, field: &'static str) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|_| PersistenceError::NumericOutOfRange {
        field,
        value: i64::MAX,
    })
}

pub(super) fn u64_from_db(field: &'static str, value: i64) -> Result<u64, PersistenceError> {
    u64::try_from(value).map_err(|_| PersistenceError::NumericOutOfRange { field, value })
}

pub(super) fn status_to_db(status: HealthStatus) -> Result<&'static str, PersistenceError> {
    match status {
        HealthStatus::Healthy => Ok("HEALTHY"),
        HealthStatus::Degraded => Ok("DEGRADED"),
        HealthStatus::Unhealthy => Ok("UNHEALTHY"),
        HealthStatus::Unknown => Ok("UNKNOWN"),
        _ => Err(PersistenceError::InvalidEnum {
            kind: "health_status",
            value: "unsupported future variant".to_owned(),
        }),
    }
}

pub(super) fn status_from_db(value: String) -> Result<HealthStatus, PersistenceError> {
    match value.as_str() {
        "HEALTHY" => Ok(HealthStatus::Healthy),
        "DEGRADED" => Ok(HealthStatus::Degraded),
        "UNHEALTHY" => Ok(HealthStatus::Unhealthy),
        "UNKNOWN" => Ok(HealthStatus::Unknown),
        _ => Err(PersistenceError::InvalidEnum {
            kind: "health_status",
            value,
        }),
    }
}

pub(super) fn check_kind_to_db(
    kind: &CheckKind,
) -> Result<(&'static str, &str, u16), PersistenceError> {
    match kind {
        CheckKind::Http {
            url,
            expected_status,
            ..
        } => Ok(("HTTP", url, *expected_status)),
        _ => Err(PersistenceError::InvalidEnum {
            kind: "check_kind",
            value: "unsupported future variant".to_owned(),
        }),
    }
}

pub(super) fn check_kind_from_db(
    kind: String,
    url: String,
    expected_status: i64,
) -> Result<CheckKind, PersistenceError> {
    match kind.as_str() {
        "HTTP" => {
            let expected_status = u16::try_from(expected_status).map_err(|_| {
                PersistenceError::NumericOutOfRange {
                    field: "checks.expected_status",
                    value: expected_status,
                }
            })?;
            Ok(CheckKind::http(url, expected_status)?)
        }
        _ => Err(PersistenceError::InvalidEnum {
            kind: "check_kind",
            value: kind,
        }),
    }
}

pub(super) fn project_from_row(row: &SqliteRow) -> Result<Project, PersistenceError> {
    Project::from_persisted(
        uuid_from_db("projects.id", row.get("id"))?,
        row.get::<String, _>("name"),
        timestamp_from_db("projects.created_at", row.get("created_at"))?,
    )
    .map_err(Into::into)
}

pub(super) fn service_from_row(row: &SqliteRow) -> Result<Service, PersistenceError> {
    Service::from_persisted(
        uuid_from_db("services.id", row.get("id"))?,
        uuid_from_db("services.project_id", row.get("project_id"))?,
        row.get::<String, _>("name"),
        timestamp_from_db("services.created_at", row.get("created_at"))?,
    )
    .map_err(Into::into)
}

pub(super) fn check_from_row(row: &SqliteRow) -> Result<Check, PersistenceError> {
    Check::from_persisted(
        uuid_from_db("checks.id", row.get("id"))?,
        uuid_from_db("checks.service_id", row.get("service_id"))?,
        check_kind_from_db(
            row.get("kind"),
            row.get("url"),
            row.get::<i64, _>("expected_status"),
        )?,
        u64_from_db("checks.interval_seconds", row.get("interval_seconds"))?,
        u64_from_db("checks.timeout_ms", row.get("timeout_ms"))?,
        row.get::<i64, _>("enabled") != 0,
        timestamp_from_db("checks.created_at", row.get("created_at"))?,
    )
    .map_err(Into::into)
}

pub(super) fn result_from_row(row: &SqliteRow) -> Result<CheckResult, PersistenceError> {
    Ok(CheckResult::from_persisted(
        uuid_from_db("check_results.id", row.get("id"))?,
        uuid_from_db("check_results.check_id", row.get("check_id"))?,
        timestamp_from_db("check_results.checked_at", row.get("checked_at"))?,
        status_from_db(row.get("status"))?,
        u64_from_db("check_results.duration_ms", row.get("duration_ms"))?,
        row.get("error"),
    ))
}
