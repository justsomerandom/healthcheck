//! Product-facing use cases composed from server repositories and runtime components.

use std::{
    future::Future,
    path::{Path, PathBuf},
};

use healthcheck_core::{Check, CheckKind, CheckResult, Project, Service};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    execution::{CheckExecutor, HttpCheckExecutor},
    persistence::{Database, PersistenceError},
    reporting::{ProjectSummary, ReportingError, ServiceHealth, StatusReporter},
    results::SqliteResultSink,
    scheduler::{Scheduler, SchedulerRunStats},
};

/// Filesystem configuration for the local HealthCheck database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("healthcheck.db"),
        }
    }
}

impl DatabaseConfig {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, ApplicationError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(ApplicationError::InvalidDatabasePath(
                "database path must not be empty".to_owned(),
            ));
        }
        if path.is_dir() {
            return Err(ApplicationError::InvalidDatabasePath(format!(
                "database path is a directory: {}",
                path.display()
            )));
        }
        Ok(Self { path })
    }

    fn url(&self) -> String {
        format!(
            "sqlite://{}",
            self.path.to_string_lossy().replace('\\', "/")
        )
    }

    fn ensure_parent_directory(&self) -> Result<(), ApplicationError> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| {
                ApplicationError::CreateDatabaseDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
        Ok(())
    }
}

/// Errors from local product use cases.
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("invalid database path: {0}")]
    InvalidDatabasePath(String),
    #[error("could not create database directory {path}: {source}")]
    CreateDatabaseDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    Domain(#[from] healthcheck_core::DomainError),
    #[error("project not found: {0}")]
    ProjectNotFound(Uuid),
    #[error("service not found: {0}")]
    ServiceNotFound(Uuid),
    #[error("check not found: {0}")]
    CheckNotFound(Uuid),
    #[error("service {service_id} does not belong to project {project_id}")]
    ServiceProjectMismatch { service_id: Uuid, project_id: Uuid },
    #[error(transparent)]
    Reporting(#[from] ReportingError),
}

/// A durable local HealthCheck application.
#[derive(Debug, Clone)]
pub struct HealthCheckApp {
    database: Database,
}

impl HealthCheckApp {
    /// Opens the configured SQLite database and applies its authoritative migrations.
    pub async fn open(config: DatabaseConfig) -> Result<Self, ApplicationError> {
        config.ensure_parent_directory()?;
        let database = Database::connect(&config.url()).await?;
        database.migrate().await?;
        Ok(Self { database })
    }

    pub async fn create_project(
        &self,
        name: impl Into<String>,
    ) -> Result<Project, ApplicationError> {
        let project = Project::new(name)?;
        self.database.projects().insert(&project).await?;
        Ok(project)
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>, ApplicationError> {
        Ok(self.database.projects().list().await?)
    }

    pub async fn create_service(
        &self,
        project_id: Uuid,
        name: impl Into<String>,
    ) -> Result<Service, ApplicationError> {
        if self
            .database
            .projects()
            .get_by_id(project_id)
            .await?
            .is_none()
        {
            return Err(ApplicationError::ProjectNotFound(project_id));
        }
        let service = Service::new(project_id, name)?;
        self.database.services().insert(&service).await?;
        Ok(service)
    }

    pub async fn list_services(&self, project_id: Uuid) -> Result<Vec<Service>, ApplicationError> {
        if self
            .database
            .projects()
            .get_by_id(project_id)
            .await?
            .is_none()
        {
            return Err(ApplicationError::ProjectNotFound(project_id));
        }
        Ok(self
            .database
            .services()
            .list_for_project(project_id)
            .await?)
    }

    /// Creates and immediately executes an enabled HTTP check, persisting its first result.
    pub async fn create_http_check(
        &self,
        service_id: Uuid,
        url: impl Into<String>,
        expected_status: u16,
        interval_seconds: u64,
        timeout_ms: u64,
    ) -> Result<(Check, CheckResult), ApplicationError> {
        if self
            .database
            .services()
            .get_by_id(service_id)
            .await?
            .is_none()
        {
            return Err(ApplicationError::ServiceNotFound(service_id));
        }
        let check = Check::new(
            service_id,
            CheckKind::http(url, expected_status)?,
            interval_seconds,
            timeout_ms,
        )?;
        self.database.checks().insert(&check).await?;
        let result = HttpCheckExecutor::new().execute(&check).await;
        self.database.check_results().insert(&result).await?;
        Ok((check, result))
    }

    pub async fn list_checks(&self, service_id: Uuid) -> Result<Vec<Check>, ApplicationError> {
        if self
            .database
            .services()
            .get_by_id(service_id)
            .await?
            .is_none()
        {
            return Err(ApplicationError::ServiceNotFound(service_id));
        }
        Ok(self.database.checks().list_for_service(service_id).await?)
    }

    pub async fn set_check_enabled(
        &self,
        check_id: Uuid,
        enabled: bool,
    ) -> Result<(), ApplicationError> {
        if self.database.checks().get_by_id(check_id).await?.is_none() {
            return Err(ApplicationError::CheckNotFound(check_id));
        }
        self.database
            .checks()
            .update_enabled(check_id, enabled)
            .await?;
        Ok(())
    }

    pub async fn project_summary(
        &self,
        project_id: Uuid,
    ) -> Result<ProjectSummary, ApplicationError> {
        Ok(StatusReporter::new(self.database.clone())
            .project_summary(project_id)
            .await?)
    }

    pub async fn service_health(
        &self,
        service_id: Uuid,
        history_limit: u32,
    ) -> Result<ServiceHealth, ApplicationError> {
        Ok(StatusReporter::new(self.database.clone())
            .service_health(service_id, history_limit)
            .await?)
    }

    /// Reloads enabled checks from SQLite and runs them until `shutdown` resolves.
    pub async fn monitor_until<F>(&self, shutdown: F) -> Result<SchedulerRunStats, ApplicationError>
    where
        F: Future<Output = ()> + Send,
    {
        let scheduler = Scheduler::new(
            HttpCheckExecutor::new(),
            SqliteResultSink::new(self.database.check_results()),
        );
        let enabled_checks = self.database.checks().list_enabled().await?;
        tracing::info!(enabled_checks = enabled_checks.len(), "starting monitor");
        for check in enabled_checks {
            scheduler.register(check).await;
        }
        let stats = scheduler.run_until_shutdown(shutdown).await;
        tracing::info!(?stats, "monitor stopped");
        Ok(stats)
    }

    pub fn database_path_exists(path: &Path) -> bool {
        path.exists()
    }
}
