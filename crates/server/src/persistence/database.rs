use std::{path::Path, str::FromStr};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};

use super::{
    CheckRepository, CheckResultRepository, PersistenceError, ProjectRepository, ServiceRepository,
};

/// Centralized SQLite connection and migration helper.
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, PersistenceError> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), PersistenceError> {
        let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_path).await?;
        migrator.run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn projects(&self) -> ProjectRepository {
        ProjectRepository::new(self.pool.clone())
    }

    pub fn services(&self) -> ServiceRepository {
        ServiceRepository::new(self.pool.clone())
    }

    pub fn checks(&self) -> CheckRepository {
        CheckRepository::new(self.pool.clone())
    }

    pub fn check_results(&self) -> CheckResultRepository {
        CheckResultRepository::new(self.pool.clone())
    }
}
