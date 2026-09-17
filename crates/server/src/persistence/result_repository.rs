use healthcheck_core::CheckResult;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{mapping, PersistenceError};

/// SQLite repository for check results.
#[derive(Debug, Clone)]
pub struct CheckResultRepository {
    pool: SqlitePool,
}

impl CheckResultRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, result: &CheckResult) -> Result<(), PersistenceError> {
        sqlx::query(
            "INSERT INTO check_results \
             (id, check_id, checked_at, status, duration_ms, error) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(mapping::uuid_to_db(result.id))
        .bind(mapping::uuid_to_db(result.check_id))
        .bind(mapping::timestamp_to_db(result.checked_at))
        .bind(mapping::status_to_db(result.status)?)
        .bind(mapping::u64_to_db(
            result.duration_ms,
            "check_results.duration_ms",
        )?)
        .bind(&result.error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn latest_for_check(
        &self,
        check_id: Uuid,
    ) -> Result<Option<CheckResult>, PersistenceError> {
        let row = sqlx::query(
            "SELECT id, check_id, checked_at, status, duration_ms, error \
             FROM check_results \
             WHERE check_id = ?1 \
             ORDER BY checked_at DESC, id DESC \
             LIMIT 1",
        )
        .bind(mapping::uuid_to_db(check_id))
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(mapping::result_from_row).transpose()
    }

    pub async fn recent_for_check(
        &self,
        check_id: Uuid,
        limit: u32,
    ) -> Result<Vec<CheckResult>, PersistenceError> {
        let rows = sqlx::query(
            "SELECT id, check_id, checked_at, status, duration_ms, error \
             FROM check_results \
             WHERE check_id = ?1 \
             ORDER BY checked_at DESC, id DESC \
             LIMIT ?2",
        )
        .bind(mapping::uuid_to_db(check_id))
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(mapping::result_from_row).collect()
    }
}
