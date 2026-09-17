use healthcheck_core::Project;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{mapping, PersistenceError};

/// SQLite repository for projects.
#[derive(Debug, Clone)]
pub struct ProjectRepository {
    pool: SqlitePool,
}

impl ProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, project: &Project) -> Result<(), PersistenceError> {
        sqlx::query("INSERT INTO projects (id, name, created_at) VALUES (?1, ?2, ?3)")
            .bind(mapping::uuid_to_db(project.id))
            .bind(&project.name)
            .bind(mapping::timestamp_to_db(project.created_at))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<Project>, PersistenceError> {
        let row = sqlx::query("SELECT id, name, created_at FROM projects WHERE id = ?1")
            .bind(mapping::uuid_to_db(id))
            .fetch_optional(&self.pool)
            .await?;

        row.as_ref().map(mapping::project_from_row).transpose()
    }

    pub async fn list(&self) -> Result<Vec<Project>, PersistenceError> {
        let rows = sqlx::query("SELECT id, name, created_at FROM projects ORDER BY created_at, id")
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(mapping::project_from_row).collect()
    }
}
