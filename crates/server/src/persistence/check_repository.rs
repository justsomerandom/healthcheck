use healthcheck_core::Check;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{mapping, PersistenceError};

/// SQLite repository for configured checks.
#[derive(Debug, Clone)]
pub struct CheckRepository {
    pool: SqlitePool,
}

impl CheckRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, check: &Check) -> Result<(), PersistenceError> {
        let (kind, url, expected_status) = mapping::check_kind_to_db(&check.kind)?;

        sqlx::query(
            "INSERT INTO checks \
             (id, service_id, kind, url, expected_status, interval_seconds, timeout_ms, enabled, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(mapping::uuid_to_db(check.id))
        .bind(mapping::uuid_to_db(check.service_id))
        .bind(kind)
        .bind(url)
        .bind(i64::from(expected_status))
        .bind(mapping::u64_to_db(
            check.interval_seconds,
            "checks.interval_seconds",
        )?)
        .bind(mapping::u64_to_db(check.timeout_ms, "checks.timeout_ms")?)
        .bind(i64::from(check.enabled))
        .bind(mapping::timestamp_to_db(check.created_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Check>, PersistenceError> {
        let row = sqlx::query(
            "SELECT id, service_id, kind, url, expected_status, interval_seconds, timeout_ms, enabled, created_at \
             FROM checks \
             WHERE id = ?1",
        )
        .bind(mapping::uuid_to_db(id))
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(mapping::check_from_row).transpose()
    }

    pub async fn list_for_service(&self, service_id: Uuid) -> Result<Vec<Check>, PersistenceError> {
        let rows = sqlx::query(
            "SELECT id, service_id, kind, url, expected_status, interval_seconds, timeout_ms, enabled, created_at \
             FROM checks \
             WHERE service_id = ?1 \
             ORDER BY created_at, id",
        )
        .bind(mapping::uuid_to_db(service_id))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(mapping::check_from_row).collect()
    }

    pub async fn list_enabled(&self) -> Result<Vec<Check>, PersistenceError> {
        let rows = sqlx::query(
            "SELECT id, service_id, kind, url, expected_status, interval_seconds, timeout_ms, enabled, created_at \
             FROM checks \
             WHERE enabled = 1 \
             ORDER BY created_at, id",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(mapping::check_from_row).collect()
    }

    pub async fn update_enabled(&self, id: Uuid, enabled: bool) -> Result<(), PersistenceError> {
        sqlx::query("UPDATE checks SET enabled = ?1 WHERE id = ?2")
            .bind(i64::from(enabled))
            .bind(mapping::uuid_to_db(id))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
