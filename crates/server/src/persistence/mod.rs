//! SQLite-backed persistence for server-owned state.

mod check_repository;
mod database;
mod error;
mod mapping;
mod project_repository;
mod result_repository;
mod service_repository;

pub use check_repository::CheckRepository;
pub use database::Database;
pub use error::PersistenceError;
pub use project_repository::ProjectRepository;
pub use result_repository::CheckResultRepository;
pub use service_repository::ServiceRepository;

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use chrono::{Duration as ChronoDuration, Utc};
    use healthcheck_core::{Check, CheckKind, CheckResult, HealthStatus, Project, Service};
    use sqlx::Row;
    use uuid::Uuid;

    use crate::{
        execution::CheckExecutor,
        results::{ResultSink, SqliteResultSink},
        scheduler::{Scheduler, SchedulerConfig},
    };

    use super::*;

    #[tokio::test]
    async fn database_initializes_and_migrations_apply_cleanly() {
        let db = test_database().await;

        db.migrate().await.expect("migrations are idempotent");

        let table_count: i64 =
            sqlx::query("SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table'")
                .fetch_one(db.pool())
                .await
                .expect("table count query succeeds")
                .get("count");

        assert!(table_count >= 5);
    }

    #[tokio::test]
    async fn project_round_trip_and_list() {
        let db = test_database().await;
        let projects = db.projects();
        let first = Project::new("Core").expect("project is valid");
        let second = Project::new("Edge").expect("project is valid");

        projects.insert(&first).await.expect("insert succeeds");
        projects.insert(&second).await.expect("insert succeeds");

        assert_eq!(projects.get_by_id(first.id).await.unwrap(), Some(first));
        assert_eq!(projects.list().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn service_round_trip_and_list_for_project() {
        let db = test_database().await;
        let (project, service, _) = persist_project_service_check(&db, true).await;
        let services = db.services();

        assert_eq!(
            services.get_by_id(service.id).await.unwrap(),
            Some(service.clone())
        );
        assert_eq!(
            services.list_for_project(project.id).await.unwrap(),
            vec![service]
        );
    }

    #[tokio::test]
    async fn http_check_round_trip_and_enabled_loading() {
        let db = test_database().await;
        let (_, service, enabled_check) = persist_project_service_check(&db, true).await;
        let mut disabled_check = Check::new(
            service.id,
            CheckKind::http("https://example.test/disabled", 204).unwrap(),
            45,
            750,
        )
        .unwrap();
        disabled_check.enabled = false;
        db.checks().insert(&disabled_check).await.unwrap();

        assert_eq!(
            db.checks().get_by_id(enabled_check.id).await.unwrap(),
            Some(enabled_check.clone())
        );
        assert_eq!(
            db.checks()
                .list_for_service(service.id)
                .await
                .unwrap()
                .len(),
            2
        );

        let enabled = db.checks().list_enabled().await.unwrap();
        assert_eq!(enabled, vec![enabled_check.clone()]);
        assert_eq!(enabled[0].interval_seconds, 30);
        assert_eq!(enabled[0].timeout_ms, 500);
        assert_eq!(
            enabled[0].kind,
            CheckKind::http("https://example.test/health", 200).unwrap()
        );
    }

    #[tokio::test]
    async fn update_enabled_controls_enabled_loading() {
        let db = test_database().await;
        let (_, _, check) = persist_project_service_check(&db, true).await;

        db.checks().update_enabled(check.id, false).await.unwrap();

        assert!(db.checks().list_enabled().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn check_result_round_trip_latest_and_recent() {
        let db = test_database().await;
        let (_, _, check) = persist_project_service_check(&db, true).await;
        let repository = db.check_results();
        let older = CheckResult::from_persisted(
            Uuid::new_v4(),
            check.id,
            Utc::now() - ChronoDuration::seconds(5),
            HealthStatus::Healthy,
            10,
            None,
        );
        let newer = CheckResult::from_persisted(
            Uuid::new_v4(),
            check.id,
            Utc::now(),
            HealthStatus::Unhealthy,
            15,
            Some("expected HTTP 200, received HTTP 503".to_owned()),
        );

        repository.insert(&older).await.unwrap();
        repository.insert(&newer).await.unwrap();

        assert_eq!(
            repository.latest_for_check(check.id).await.unwrap(),
            Some(newer.clone())
        );
        assert_eq!(
            repository.recent_for_check(check.id, 2).await.unwrap(),
            vec![newer, older]
        );
    }

    #[tokio::test]
    async fn invalid_persisted_enum_value_fails_cleanly() {
        let db = test_database().await;
        let (_, _, check) = persist_project_service_check(&db, true).await;
        let mut connection = db.pool().acquire().await.unwrap();

        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO check_results (id, check_id, checked_at, status, duration_ms, error) \
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(check.id.to_string())
        .bind(Utc::now().to_rfc3339())
        .bind("BOUNCY")
        .bind(1_i64)
        .execute(&mut *connection)
        .await
        .unwrap();
        sqlx::query("PRAGMA ignore_check_constraints = OFF")
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);

        let error = db
            .check_results()
            .latest_for_check(check.id)
            .await
            .expect_err("invalid enum should fail");

        assert!(matches!(error, PersistenceError::InvalidEnum { .. }));
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced() {
        let db = test_database().await;
        let service = Service::new(Uuid::new_v4(), "Orphan").unwrap();

        let error = db
            .services()
            .insert(&service)
            .await
            .expect_err("missing project should violate FK");

        assert!(matches!(error, PersistenceError::Database(_)));
    }

    #[tokio::test]
    async fn sqlite_result_sink_records_results() {
        let db = test_database().await;
        let (_, _, check) = persist_project_service_check(&db, true).await;
        let sink = SqliteResultSink::new(db.check_results());
        let result = CheckResult::new(check.id, HealthStatus::Healthy, 8, None);

        sink.record(result.clone()).await.expect("record succeeds");

        assert_eq!(
            db.check_results().latest_for_check(check.id).await.unwrap(),
            Some(result)
        );
    }

    #[tokio::test]
    async fn scheduler_can_bootstrap_from_enabled_checks_and_persist_results() {
        let db = test_database().await;
        let (_, _, check) = persist_project_service_check(&db, true).await;
        let scheduler = Scheduler::with_config(
            PersistedFakeExecutor,
            SqliteResultSink::new(db.check_results()),
            SchedulerConfig {
                tick_interval: Duration::from_millis(5),
                interval_unit: Duration::from_millis(20),
            },
        );

        for check in db.checks().list_enabled().await.unwrap() {
            scheduler.register(check).await;
        }

        scheduler
            .run_until_shutdown(tokio::time::sleep(Duration::from_millis(40)))
            .await;

        assert!(db
            .check_results()
            .latest_for_check(check.id)
            .await
            .unwrap()
            .is_some());
    }

    async fn test_database() -> Database {
        fs::create_dir_all("target/sqlite-tests").expect("test database dir should exist");
        let url = format!("sqlite://target/sqlite-tests/{}.db", Uuid::new_v4());
        let db = Database::connect(&url).await.expect("database connects");
        db.migrate().await.expect("migrations apply");
        db
    }

    async fn persist_project_service_check(
        db: &Database,
        enabled: bool,
    ) -> (Project, Service, Check) {
        let project = Project::new("Platform").expect("project is valid");
        let service = Service::new(project.id, "API").expect("service is valid");
        let mut check = Check::new(
            service.id,
            CheckKind::http("https://example.test/health", 200).unwrap(),
            30,
            500,
        )
        .unwrap();
        check.enabled = enabled;

        db.projects().insert(&project).await.unwrap();
        db.services().insert(&service).await.unwrap();
        db.checks().insert(&check).await.unwrap();

        (project, service, check)
    }

    #[derive(Debug)]
    struct PersistedFakeExecutor;

    impl CheckExecutor for PersistedFakeExecutor {
        async fn execute<'a>(&'a self, check: &'a Check) -> CheckResult {
            CheckResult::new(check.id, HealthStatus::Healthy, 1, None)
        }
    }
}
