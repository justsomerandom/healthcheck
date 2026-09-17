use healthcheck_core::Service;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{mapping, PersistenceError};

/// SQLite repository for services.
#[derive(Debug, Clone)]
pub struct ServiceRepository {
    pool: SqlitePool,
}

impl ServiceRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, service: &Service) -> Result<(), PersistenceError> {
        sqlx::query(
            "INSERT INTO services (id, project_id, name, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(mapping::uuid_to_db(service.id))
        .bind(mapping::uuid_to_db(service.project_id))
        .bind(&service.name)
        .bind(mapping::timestamp_to_db(service.created_at))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Service>, PersistenceError> {
        let row =
            sqlx::query("SELECT id, project_id, name, created_at FROM services WHERE id = ?1")
                .bind(mapping::uuid_to_db(id))
                .fetch_optional(&self.pool)
                .await?;

        row.as_ref().map(mapping::service_from_row).transpose()
    }

    pub async fn list_for_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Service>, PersistenceError> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, created_at \
             FROM services \
             WHERE project_id = ?1 \
             ORDER BY created_at, id",
        )
        .bind(mapping::uuid_to_db(project_id))
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(mapping::service_from_row).collect()
    }
}
