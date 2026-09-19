//! Read-model queries for human-facing local monitoring reports.

use healthcheck_core::{Check, CheckResult, HealthStatus, Project, Service};
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::{Database, PersistenceError};

#[derive(Debug, Error)]
pub enum ReportingError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("project not found: {0}")]
    ProjectNotFound(Uuid),
    #[error("service not found: {0}")]
    ServiceNotFound(Uuid),
}

#[derive(Debug, Clone)]
pub struct CheckHealth {
    pub check: Check,
    pub latest_result: Option<CheckResult>,
    pub recent_history: Vec<CheckResult>,
}

#[derive(Debug, Clone)]
pub struct ServiceHealth {
    pub service: Service,
    pub status: HealthStatus,
    pub checks: Vec<CheckHealth>,
}

#[derive(Debug, Clone)]
pub struct ProjectSummary {
    pub project: Project,
    pub status: HealthStatus,
    pub services: Vec<ServiceHealth>,
}

/// Composes repository reads into reporting-focused structures without embedding SQL in the CLI.
#[derive(Debug, Clone)]
pub struct StatusReporter {
    database: Database,
}

impl StatusReporter {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn service_health(
        &self,
        service_id: Uuid,
        history_limit: u32,
    ) -> Result<ServiceHealth, ReportingError> {
        let service = self
            .database
            .services()
            .get_by_id(service_id)
            .await?
            .ok_or(ReportingError::ServiceNotFound(service_id))?;
        self.health_for_service(service, history_limit).await
    }

    pub async fn project_summary(
        &self,
        project_id: Uuid,
    ) -> Result<ProjectSummary, ReportingError> {
        let project = self
            .database
            .projects()
            .get_by_id(project_id)
            .await?
            .ok_or(ReportingError::ProjectNotFound(project_id))?;
        let mut services = Vec::new();
        for service in self
            .database
            .services()
            .list_for_project(project_id)
            .await?
        {
            services.push(self.health_for_service(service, 0).await?);
        }
        let status = aggregate_status(services.iter().map(|service| service.status));
        Ok(ProjectSummary {
            project,
            status,
            services,
        })
    }

    async fn health_for_service(
        &self,
        service: Service,
        history_limit: u32,
    ) -> Result<ServiceHealth, ReportingError> {
        let mut checks = Vec::new();
        for check in self.database.checks().list_for_service(service.id).await? {
            let latest_result = self
                .database
                .check_results()
                .latest_for_check(check.id)
                .await?;
            let recent_history = if history_limit == 0 {
                Vec::new()
            } else {
                self.database
                    .check_results()
                    .recent_for_check(check.id, history_limit)
                    .await?
            };
            checks.push(CheckHealth {
                check,
                latest_result,
                recent_history,
            });
        }
        let status = aggregate_status(checks.iter().filter(|check| check.check.enabled).map(
            |check| {
                check
                    .latest_result
                    .as_ref()
                    .map(|result| result.status)
                    .unwrap_or(HealthStatus::Unknown)
            },
        ));
        Ok(ServiceHealth {
            service,
            status,
            checks,
        })
    }
}

fn aggregate_status(statuses: impl Iterator<Item = HealthStatus>) -> HealthStatus {
    let statuses: Vec<_> = statuses.collect();
    if statuses.is_empty() {
        return HealthStatus::Unknown;
    }
    if statuses.contains(&HealthStatus::Unhealthy) {
        HealthStatus::Unhealthy
    } else if statuses.contains(&HealthStatus::Degraded) {
        HealthStatus::Degraded
    } else if statuses.contains(&HealthStatus::Unknown) {
        HealthStatus::Unknown
    } else {
        HealthStatus::Healthy
    }
}
