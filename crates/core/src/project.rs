use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::DomainError;

/// A bundle of related services monitored together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Project {
    /// Creates a project with a generated id and creation timestamp.
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        Ok(Self {
            id: Uuid::new_v4(),
            name: validate_name("project", name)?,
            created_at: Utc::now(),
        })
    }

    /// Reconstructs a project from previously persisted domain fields.
    pub fn from_persisted(
        id: Uuid,
        name: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            name: validate_name("project", name)?,
            created_at,
        })
    }
}

pub(crate) fn validate_name(
    entity: &'static str,
    name: impl Into<String>,
) -> Result<String, DomainError> {
    let name = name.into().trim().to_owned();

    if name.is_empty() {
        return Err(DomainError::EmptyName { entity });
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_project_with_valid_name() {
        let project = Project::new("Platform").expect("project should be valid");

        assert_eq!(project.name, "Platform");
        assert_ne!(project.id, Uuid::nil());
    }

    #[test]
    fn trims_project_name() {
        let project = Project::new(" Platform ").expect("project should be valid");

        assert_eq!(project.name, "Platform");
    }

    #[test]
    fn rejects_empty_project_name() {
        let error = Project::new("   ").expect_err("blank project name should fail");

        assert_eq!(error, DomainError::EmptyName { entity: "project" });
    }

    #[test]
    fn reconstructs_persisted_project() {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let project =
            Project::from_persisted(id, "Persisted", created_at).expect("project is valid");

        assert_eq!(project.id, id);
        assert_eq!(project.created_at, created_at);
    }
}
