use std::future::Future;

use healthcheck_core::{Check, CheckResult};

/// Executes one configured health check and returns the observed result.
pub trait CheckExecutor {
    fn execute<'a>(&'a self, check: &'a Check) -> impl Future<Output = CheckResult> + Send + 'a;
}
