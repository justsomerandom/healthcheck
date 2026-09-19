use std::{fs, path::PathBuf, time::Duration};

use healthcheck_core::HealthStatus;
use healthcheck_server::application::{DatabaseConfig, HealthCheckApp};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use uuid::Uuid;

#[tokio::test]
async fn durable_healthy_workflow_reloads_scheduler_and_stops_cleanly() {
    let database_path = test_database_path();
    let (url, _server) = spawn_http_server(200, None, 8).await;
    let app = HealthCheckApp::open(DatabaseConfig::new(&database_path).unwrap())
        .await
        .unwrap();
    let project = app.create_project("Platform").await.unwrap();
    let service = app.create_service(project.id, "API").await.unwrap();
    let (check, initial) = app
        .create_http_check(service.id, url, 200, 1, 500)
        .await
        .unwrap();

    assert_eq!(initial.status, HealthStatus::Healthy);
    assert_eq!(app.list_projects().await.unwrap(), vec![project.clone()]);
    assert_eq!(
        app.list_services(project.id).await.unwrap(),
        vec![service.clone()]
    );
    assert_eq!(
        app.list_checks(service.id).await.unwrap(),
        vec![check.clone()]
    );
    drop(app);

    let restarted = HealthCheckApp::open(DatabaseConfig::new(&database_path).unwrap())
        .await
        .unwrap();
    let stats = restarted
        .monitor_until(tokio::time::sleep(Duration::from_millis(1_250)))
        .await
        .unwrap();
    assert!(
        stats.completed_executions >= 1,
        "scheduler should reload and execute persisted enabled check"
    );

    let health = restarted.service_health(service.id, 10).await.unwrap();
    assert_eq!(health.status, HealthStatus::Healthy);
    assert!(
        health.checks[0].recent_history.len() >= 2,
        "initial result and recurring results persist"
    );
    assert_eq!(
        restarted.project_summary(project.id).await.unwrap().status,
        HealthStatus::Healthy
    );
}

#[tokio::test]
async fn unhealthy_timeout_history_and_disabled_check_are_reported() {
    let database_path = test_database_path();
    let (unhealthy_url, _unhealthy_server) = spawn_http_server(503, None, 2).await;
    let (slow_url, _slow_server) =
        spawn_http_server(200, Some(Duration::from_millis(150)), 2).await;
    let app = HealthCheckApp::open(DatabaseConfig::new(&database_path).unwrap())
        .await
        .unwrap();
    let project = app.create_project("Edge").await.unwrap();
    let service = app.create_service(project.id, "Gateway").await.unwrap();
    let (unhealthy_check, unhealthy) = app
        .create_http_check(service.id, unhealthy_url, 200, 1, 500)
        .await
        .unwrap();
    let (timeout_check, timeout) = app
        .create_http_check(service.id, slow_url, 200, 1, 20)
        .await
        .unwrap();

    assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
    assert_eq!(timeout.status, HealthStatus::Unhealthy);
    assert!(timeout.error.unwrap().contains("timed out"));
    app.set_check_enabled(unhealthy_check.id, false)
        .await
        .unwrap();
    app.set_check_enabled(timeout_check.id, false)
        .await
        .unwrap();
    let stats = app
        .monitor_until(tokio::time::sleep(Duration::from_millis(350)))
        .await
        .unwrap();
    assert_eq!(
        stats.completed_executions, 0,
        "disabled checks must not be reloaded"
    );

    let health = app.service_health(service.id, 10).await.unwrap();
    assert_eq!(
        health.status,
        HealthStatus::Unknown,
        "a service with no enabled checks is unknown"
    );
    assert!(health.checks.iter().all(|check| !check.check.enabled));
    assert!(health
        .checks
        .iter()
        .all(|check| check.recent_history.len() == 1));
}

fn test_database_path() -> PathBuf {
    let directory = PathBuf::from("target/local-product-tests");
    fs::create_dir_all(&directory).unwrap();
    directory.join(format!("{}.db", Uuid::new_v4()))
}

async fn spawn_http_server(
    status: u16,
    delay: Option<Duration>,
    request_count: usize,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        for _ in 0..request_count {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer).await;
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
            let response =
                format!("HTTP/1.1 {status} OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    (format!("http://{address}/health"), handle)
}
