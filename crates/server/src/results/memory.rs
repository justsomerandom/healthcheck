use std::sync::Arc;

use healthcheck_core::CheckResult;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{ResultSink, ResultSinkError};

/// Concurrent in-memory result sink for tests and early runtime validation.
#[derive(Debug, Clone, Default)]
pub struct InMemoryResultSink {
    results: Arc<Mutex<Vec<CheckResult>>>,
}

impl InMemoryResultSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn all_results(&self) -> Vec<CheckResult> {
        self.results.lock().await.clone()
    }

    pub async fn results_for_check(&self, check_id: Uuid) -> Vec<CheckResult> {
        self.results
            .lock()
            .await
            .iter()
            .filter(|result| result.check_id == check_id)
            .cloned()
            .collect()
    }

    pub async fn latest_for_check(&self, check_id: Uuid) -> Option<CheckResult> {
        self.results
            .lock()
            .await
            .iter()
            .rev()
            .find(|result| result.check_id == check_id)
            .cloned()
    }

    pub async fn clear(&self) {
        self.results.lock().await.clear();
    }
}

impl ResultSink for InMemoryResultSink {
    async fn record(&self, result: CheckResult) -> Result<(), ResultSinkError> {
        self.results.lock().await.push(result);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use healthcheck_core::HealthStatus;

    use super::*;

    #[tokio::test]
    async fn stores_results_in_insertion_order() {
        let sink = InMemoryResultSink::new();
        let check_id = Uuid::new_v4();
        let first = CheckResult::new(check_id, HealthStatus::Healthy, 10, None);
        let second = CheckResult::new(
            check_id,
            HealthStatus::Unhealthy,
            20,
            Some("failed".to_owned()),
        );

        sink.record(first.clone()).await.expect("record succeeds");
        sink.record(second.clone()).await.expect("record succeeds");

        assert_eq!(
            sink.all_results().await,
            vec![first.clone(), second.clone()]
        );
        assert_eq!(sink.results_for_check(check_id).await, vec![first, second]);
    }

    #[tokio::test]
    async fn returns_latest_result_for_check() {
        let sink = InMemoryResultSink::new();
        let check_id = Uuid::new_v4();
        let other_check_id = Uuid::new_v4();
        let first = CheckResult::new(check_id, HealthStatus::Healthy, 10, None);
        let latest = CheckResult::new(check_id, HealthStatus::Unhealthy, 15, None);
        let other = CheckResult::new(other_check_id, HealthStatus::Healthy, 5, None);

        sink.record(first).await.expect("record succeeds");
        sink.record(other.clone()).await.expect("record succeeds");
        sink.record(latest.clone()).await.expect("record succeeds");

        assert_eq!(sink.latest_for_check(check_id).await, Some(latest));
        assert_eq!(sink.latest_for_check(other_check_id).await, Some(other));
    }

    #[tokio::test]
    async fn clears_results() {
        let sink = InMemoryResultSink::new();
        let check_id = Uuid::new_v4();

        sink.record(CheckResult::new(check_id, HealthStatus::Healthy, 10, None))
            .await
            .expect("record succeeds");

        sink.clear().await;

        assert!(sink.all_results().await.is_empty());
    }
}
