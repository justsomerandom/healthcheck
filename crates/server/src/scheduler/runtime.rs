use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use healthcheck_core::Check;
use tokio::{sync::Mutex, task::JoinSet, time::MissedTickBehavior};
use uuid::Uuid;

use crate::{execution::CheckExecutor, results::ResultSink};

use super::state::SchedulerState;

/// Runtime configuration for the in-memory scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub tick_interval: Duration,
    pub interval_unit: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(250),
            interval_unit: Duration::from_secs(1),
        }
    }
}

/// Summary of a scheduler run after shutdown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchedulerRunStats {
    pub completed_executions: u64,
    pub sink_errors: u64,
    pub task_failures: u64,
}

/// Fixed-delay in-memory scheduler for configured checks.
#[derive(Debug)]
pub struct Scheduler<E, S> {
    executor: Arc<E>,
    result_sink: Arc<S>,
    state: Arc<Mutex<SchedulerState>>,
    config: SchedulerConfig,
}

impl<E, S> Scheduler<E, S> {
    pub fn new(executor: E, result_sink: S) -> Self {
        Self::with_config(executor, result_sink, SchedulerConfig::default())
    }

    pub fn with_config(executor: E, result_sink: S, config: SchedulerConfig) -> Self {
        Self {
            executor: Arc::new(executor),
            result_sink: Arc::new(result_sink),
            state: Arc::new(Mutex::new(SchedulerState::new())),
            config,
        }
    }

    pub async fn register(&self, check: Check) {
        self.state.lock().await.register(check, Instant::now());
    }
}

impl<E, S> Scheduler<E, S>
where
    E: CheckExecutor + Send + Sync + 'static,
    S: ResultSink + Send + Sync + 'static,
{
    /// Runs until the provided shutdown future resolves.
    ///
    /// Scheduling uses fixed-delay semantics: after a check completes, its next
    /// due time is completion time plus `check.interval_seconds`.
    pub async fn run_until_shutdown<F>(&self, shutdown: F) -> SchedulerRunStats
    where
        F: Future<Output = ()> + Send,
    {
        let mut stats = SchedulerRunStats::default();
        let mut ticker = tokio::time::interval(self.config.tick_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut running_tasks = JoinSet::new();
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    break;
                }
                _ = ticker.tick() => {
                    self.spawn_due_checks(&mut running_tasks).await;
                }
                completed = running_tasks.join_next(), if !running_tasks.is_empty() => {
                    Self::handle_task_completion(
                        completed,
                        &self.state,
                        self.config.interval_unit,
                        &mut stats,
                    )
                    .await;
                }
            }
        }

        while let Some(completed) = running_tasks.join_next().await {
            Self::handle_task_completion(
                Some(completed),
                &self.state,
                self.config.interval_unit,
                &mut stats,
            )
            .await;
        }

        stats
    }

    async fn spawn_due_checks(&self, running_tasks: &mut JoinSet<ExecutionCompletion>) {
        let checks = self.state.lock().await.claim_due_checks(Instant::now());

        for check in checks {
            let executor = Arc::clone(&self.executor);
            let result_sink = Arc::clone(&self.result_sink);

            running_tasks.spawn(async move {
                let check_id = check.id;
                let result = executor.execute(&check).await;
                let recorded = result_sink.record(result).await.is_ok();

                ExecutionCompletion {
                    check_id,
                    completed_at: Instant::now(),
                    recorded,
                }
            });
        }
    }

    async fn handle_task_completion(
        completed: Option<Result<ExecutionCompletion, tokio::task::JoinError>>,
        state: &Mutex<SchedulerState>,
        interval_unit: Duration,
        stats: &mut SchedulerRunStats,
    ) {
        match completed {
            Some(Ok(completion)) => {
                state.lock().await.complete(
                    completion.check_id,
                    completion.completed_at,
                    interval_unit,
                );
                stats.completed_executions += 1;

                if !completion.recorded {
                    stats.sink_errors += 1;
                }
            }
            Some(Err(_)) => {
                stats.task_failures += 1;
            }
            None => {}
        }
    }
}

#[derive(Debug)]
struct ExecutionCompletion {
    check_id: Uuid,
    completed_at: Instant,
    recorded: bool,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::Duration,
    };

    use healthcheck_core::{CheckKind, CheckResult, DomainError, HealthStatus};
    use tokio::sync::{oneshot, Mutex};

    use crate::results::InMemoryResultSink;

    use super::*;

    #[tokio::test]
    async fn repeated_execution_runs_check_multiple_times() {
        let executor = FakeExecutor::new(Duration::from_millis(1));
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink.clone());
        let check = http_check(1, true).expect("check should be valid");

        scheduler.register(check.clone()).await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(140)))
            .await;

        assert!(sink.results_for_check(check.id).await.len() >= 2);
    }

    #[tokio::test]
    async fn different_intervals_execute_at_different_frequencies() {
        let executor = FakeExecutor::new(Duration::from_millis(1));
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink.clone());
        let fast = http_check(1, true).expect("check should be valid");
        let slow = http_check(3, true).expect("check should be valid");

        scheduler.register(fast.clone()).await;
        scheduler.register(slow.clone()).await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(130)))
            .await;

        let fast_count = sink.results_for_check(fast.id).await.len();
        let slow_count = sink.results_for_check(slow.id).await.len();

        assert!(fast_count > slow_count);
        assert!(slow_count >= 2);
    }

    #[tokio::test]
    async fn disabled_checks_do_not_execute() {
        let executor = FakeExecutor::new(Duration::from_millis(1));
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink.clone());
        let check = http_check(1, false).expect("check should be valid");

        scheduler.register(check.clone()).await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(60)))
            .await;

        assert!(sink.results_for_check(check.id).await.is_empty());
    }

    #[tokio::test]
    async fn distinct_checks_can_execute_concurrently() {
        let executor = FakeExecutor::new(Duration::from_millis(60));
        let metrics = executor.metrics();
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink);

        scheduler
            .register(http_check(1, true).expect("check should be valid"))
            .await;
        scheduler
            .register(http_check(1, true).expect("check should be valid"))
            .await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(25)))
            .await;

        assert!(metrics.lock().await.max_global_running >= 2);
    }

    #[tokio::test]
    async fn same_check_never_overlaps_with_itself() {
        let executor = FakeExecutor::new(Duration::from_millis(50));
        let metrics = executor.metrics();
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink.clone());
        let check = http_check(1, true).expect("check should be valid");

        scheduler.register(check.clone()).await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(130)))
            .await;

        let metrics = metrics.lock().await;
        assert_eq!(metrics.max_running_for_check(check.id), 1);
        assert!(sink.results_for_check(check.id).await.len() <= 3);
    }

    #[tokio::test]
    async fn completed_results_reach_sink() {
        let executor = FakeExecutor::new(Duration::from_millis(1));
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink.clone());
        let check = http_check(1, true).expect("check should be valid");

        scheduler.register(check.clone()).await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(30)))
            .await;

        let results = sink.results_for_check(check.id).await;
        assert!(!results.is_empty());
        assert!(results.iter().all(|result| result.check_id == check.id));
    }

    #[tokio::test]
    async fn shutdown_signal_exits_cleanly() {
        let executor = FakeExecutor::new(Duration::from_millis(1));
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink);
        let check = http_check(1, true).expect("check should be valid");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        scheduler.register(check).await;

        let run = async {
            scheduler
                .run_until_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        };

        shutdown_tx.send(()).expect("shutdown receiver is active");
        tokio::time::timeout(Duration::from_millis(100), run)
            .await
            .expect("scheduler should stop promptly");
    }

    #[tokio::test]
    async fn failing_check_result_does_not_stop_unrelated_checks() {
        let failing = http_check(1, true).expect("check should be valid");
        let healthy = http_check(1, true).expect("check should be valid");
        let executor = FakeExecutor::new(Duration::from_millis(1)).with_unhealthy_check(failing.id);
        let sink = InMemoryResultSink::new();
        let scheduler = test_scheduler(executor, sink.clone());

        scheduler.register(failing.clone()).await;
        scheduler.register(healthy.clone()).await;
        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(70)))
            .await;

        assert!(sink
            .results_for_check(failing.id)
            .await
            .iter()
            .any(|result| { result.status == HealthStatus::Unhealthy }));
        assert!(sink.results_for_check(healthy.id).await.len() >= 2);
    }

    fn test_scheduler(
        executor: FakeExecutor,
        sink: InMemoryResultSink,
    ) -> Scheduler<FakeExecutor, InMemoryResultSink> {
        Scheduler::with_config(
            executor,
            sink,
            SchedulerConfig {
                tick_interval: Duration::from_millis(5),
                interval_unit: Duration::from_millis(20),
            },
        )
    }

    fn http_check(interval_seconds: u64, enabled: bool) -> Result<Check, DomainError> {
        let mut check = Check::new(
            uuid::Uuid::new_v4(),
            CheckKind::http("http://example.test/health", 200)?,
            interval_seconds,
            1_000,
        )?;
        check.enabled = enabled;
        Ok(check)
    }

    #[derive(Debug, Clone)]
    struct FakeExecutor {
        delay: Duration,
        unhealthy_checks: Arc<HashSet<Uuid>>,
        metrics: Arc<Mutex<FakeExecutorMetrics>>,
    }

    impl FakeExecutor {
        fn new(delay: Duration) -> Self {
            Self {
                delay,
                unhealthy_checks: Arc::new(HashSet::new()),
                metrics: Arc::new(Mutex::new(FakeExecutorMetrics::default())),
            }
        }

        fn with_unhealthy_check(mut self, check_id: Uuid) -> Self {
            self.unhealthy_checks = Arc::new(HashSet::from([check_id]));
            self
        }

        fn metrics(&self) -> Arc<Mutex<FakeExecutorMetrics>> {
            Arc::clone(&self.metrics)
        }
    }

    impl CheckExecutor for FakeExecutor {
        async fn execute<'a>(&'a self, check: &'a Check) -> CheckResult {
            {
                let mut metrics = self.metrics.lock().await;
                metrics.start(check.id);
            }

            tokio::time::sleep(self.delay).await;

            {
                let mut metrics = self.metrics.lock().await;
                metrics.finish(check.id);
            }

            if self.unhealthy_checks.contains(&check.id) {
                CheckResult::new(
                    check.id,
                    HealthStatus::Unhealthy,
                    self.delay.as_millis() as u64,
                    Some("simulated failure".to_owned()),
                )
            } else {
                CheckResult::new(
                    check.id,
                    HealthStatus::Healthy,
                    self.delay.as_millis() as u64,
                    None,
                )
            }
        }
    }

    #[derive(Debug, Default)]
    struct FakeExecutorMetrics {
        running_by_check: HashMap<Uuid, u64>,
        max_running_by_check: HashMap<Uuid, u64>,
        global_running: u64,
        max_global_running: u64,
    }

    impl FakeExecutorMetrics {
        fn start(&mut self, check_id: Uuid) {
            let running = self.running_by_check.entry(check_id).or_default();
            *running += 1;
            self.max_running_by_check
                .entry(check_id)
                .and_modify(|max| *max = (*max).max(*running))
                .or_insert(*running);

            self.global_running += 1;
            self.max_global_running = self.max_global_running.max(self.global_running);
        }

        fn finish(&mut self, check_id: Uuid) {
            if let Some(running) = self.running_by_check.get_mut(&check_id) {
                *running = running.saturating_sub(1);
            }

            self.global_running = self.global_running.saturating_sub(1);
        }

        fn max_running_for_check(&self, check_id: Uuid) -> u64 {
            self.max_running_by_check
                .get(&check_id)
                .copied()
                .unwrap_or_default()
        }
    }
}
