use std::time::{Duration, Instant};

use healthcheck_core::{Check, CheckKind, CheckResult, HealthStatus};

use super::CheckExecutor;

/// Reusable HTTP health-check executor.
#[derive(Debug, Clone)]
pub struct HttpCheckExecutor {
    client: reqwest::Client,
}

impl HttpCheckExecutor {
    /// Creates an HTTP executor with a reusable reqwest client.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    async fn execute_http(&self, check: &Check, url: &str, expected_status: u16) -> CheckResult {
        let started_at = Instant::now();
        let timeout = Duration::from_millis(check.timeout_ms);
        let response = self.client.get(url).timeout(timeout).send().await;
        let duration_ms = elapsed_millis(started_at);

        match response {
            Ok(response) => {
                let received_status = response.status().as_u16();

                if received_status == expected_status {
                    CheckResult::new(check.id, HealthStatus::Healthy, duration_ms, None)
                } else {
                    CheckResult::new(
                        check.id,
                        HealthStatus::Unhealthy,
                        duration_ms,
                        Some(format!(
                            "expected HTTP {expected_status}, received HTTP {received_status}"
                        )),
                    )
                }
            }
            Err(error) if error.is_timeout() => CheckResult::new(
                check.id,
                HealthStatus::Unhealthy,
                duration_ms,
                Some(format!(
                    "HTTP request timed out after {} ms",
                    check.timeout_ms
                )),
            ),
            Err(error) => CheckResult::new(
                check.id,
                HealthStatus::Unhealthy,
                duration_ms,
                Some(format!("HTTP request failed: {error}")),
            ),
        }
    }

    fn unsupported_check_result(check: &Check) -> CheckResult {
        CheckResult::new(
            check.id,
            HealthStatus::Unknown,
            0,
            Some("unsupported check kind for HTTP executor".to_owned()),
        )
    }
}

impl Default for HttpCheckExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckExecutor for HttpCheckExecutor {
    async fn execute<'a>(&'a self, check: &'a Check) -> CheckResult {
        match &check.kind {
            CheckKind::Http {
                url,
                expected_status,
                ..
            } => self.execute_http(check, url, *expected_status).await,
            _ => Self::unsupported_check_result(check),
        }
    }
}

fn elapsed_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use healthcheck_core::{CheckKind, DomainError};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    use super::*;

    #[tokio::test]
    async fn expected_http_status_returns_healthy() {
        let (url, _server) = spawn_http_server(200, None, 1).await;
        let check = http_check(&url, 200, 1_000).expect("check should be valid");
        let executor = HttpCheckExecutor::new();

        let result = executor.execute(&check).await;

        assert_eq!(result.check_id, check.id);
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.error, None);
    }

    #[tokio::test]
    async fn unexpected_http_status_returns_unhealthy() {
        let (url, _server) = spawn_http_server(503, None, 1).await;
        let check = http_check(&url, 200, 1_000).expect("check should be valid");
        let executor = HttpCheckExecutor::new();

        let result = executor.execute(&check).await;

        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(
            result.error.as_deref(),
            Some("expected HTTP 200, received HTTP 503")
        );
    }

    #[tokio::test]
    async fn timeout_returns_unhealthy_without_panic() {
        let (url, _server) = spawn_http_server(200, Some(Duration::from_millis(150)), 1).await;
        let check = http_check(&url, 200, 25).expect("check should be valid");
        let executor = HttpCheckExecutor::new();

        let result = executor.execute(&check).await;

        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(
            result.error.as_deref(),
            Some("HTTP request timed out after 25 ms")
        );
    }

    #[tokio::test]
    async fn connection_failure_returns_unhealthy_without_panic() {
        let (url, _server) = spawn_closing_server().await;
        let check = http_check(&url, 200, 1_000).expect("check should be valid");
        let executor = HttpCheckExecutor::new();

        let result = executor.execute(&check).await;

        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert!(result
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("HTTP request failed:")));
    }

    #[tokio::test]
    async fn duration_is_populated() {
        let (url, _server) = spawn_http_server(200, Some(Duration::from_millis(25)), 1).await;
        let check = http_check(&url, 200, 1_000).expect("check should be valid");
        let executor = HttpCheckExecutor::new();

        let result = executor.execute(&check).await;

        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.duration_ms > 0);
    }

    #[tokio::test]
    async fn multiple_executions_reuse_one_executor() {
        let (url, _server) = spawn_http_server(200, None, 2).await;
        let check = http_check(&url, 200, 1_000).expect("check should be valid");
        let executor = HttpCheckExecutor::new();

        let first = executor.execute(&check).await;
        let second = executor.execute(&check).await;

        assert_eq!(first.status, HealthStatus::Healthy);
        assert_eq!(second.status, HealthStatus::Healthy);
        assert_eq!(first.check_id, check.id);
        assert_eq!(second.check_id, check.id);
    }

    async fn spawn_http_server(
        status: u16,
        delay: Option<Duration>,
        request_count: usize,
    ) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let address = listener
            .local_addr()
            .expect("test server should have address");

        let handle = tokio::spawn(async move {
            for _ in 0..request_count {
                let (mut stream, _) = listener.accept().await.expect("request should connect");
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer).await;

                if let Some(delay) = delay {
                    tokio::time::sleep(delay).await;
                }

                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        (format!("http://{address}/health"), handle)
    }

    async fn spawn_closing_server() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("closing test server should bind");
        let address = listener
            .local_addr()
            .expect("closing test server should have address");

        let handle = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.expect("request should connect");
        });

        (format!("http://{address}/health"), handle)
    }

    fn http_check(url: &str, expected_status: u16, timeout_ms: u64) -> Result<Check, DomainError> {
        Check::new(
            uuid::Uuid::new_v4(),
            CheckKind::http(url, expected_status)?,
            30,
            timeout_ms,
        )
    }
}
