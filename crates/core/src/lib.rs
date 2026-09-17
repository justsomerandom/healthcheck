//! Shared HealthCheck domain types.

mod check;
mod error;
mod project;
mod result;
mod service;

pub use check::{Check, CheckKind, HealthStatus};
pub use error::DomainError;
pub use project::Project;
pub use result::CheckResult;
pub use service::Service;
