use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{project::validate_name, DomainError};

/// One monitored component within a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Service {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Service {
    /// Creates a service with a generated id and creation timestamp.
    pub fn new(project_id: Uuid, name: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self {
            id: Uuid::new_v4(),
            project_id,
            name: validate_name("service", name)?,
            created_at: Utc::now(),
        })
    }

    /// Reconstructs a service from previously persisted domain fields.
    pub fn from_persisted(
        id: Uuid,
        project_id: Uuid,
        name: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            project_id,
            name: validate_name("service", name)?,
            created_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_service_with_valid_name() {
        let project_id = Uuid::new_v4();
        let service = Service::new(project_id, "API").expect("service should be valid");

        assert_eq!(service.project_id, project_id);
        assert_eq!(service.name, "API");
        assert_ne!(service.id, Uuid::nil());
    }

    #[test]
    fn rejects_empty_service_name() {
        let error =
            Service::new(Uuid::new_v4(), "\t\n").expect_err("blank service name should fail");

        assert_eq!(error, DomainError::EmptyName { entity: "service" });
    }

    #[test]
    fn reconstructs_persisted_service() {
        let id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let created_at = Utc::now();
        let service =
            Service::from_persisted(id, project_id, "API", created_at).expect("service is valid");

        assert_eq!(service.id, id);
        assert_eq!(service.project_id, project_id);
        assert_eq!(service.created_at, created_at);
    }
}
