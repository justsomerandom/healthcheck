use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use healthcheck_core::{CheckKind, CheckResult, HealthStatus};
use healthcheck_server::{
    application::{DatabaseConfig, HealthCheckApp},
    reporting::{ProjectSummary, ServiceHealth},
};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "healthcheck", version, about = "Local HTTP service monitoring")]
struct Cli {
    #[arg(long, global = true, default_value = "healthcheck.db")]
    database: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Debug, Subcommand)]
enum Command {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    Check {
        #[command(subcommand)]
        command: CheckCommand,
    },
    Monitor,
}
#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Create { name: String },
    List,
    Summary { project_id: Uuid },
}
#[derive(Debug, Subcommand)]
enum ServiceCommand {
    Create {
        project_id: Uuid,
        name: String,
    },
    List {
        project_id: Uuid,
    },
    Health {
        service_id: Uuid,
        #[arg(long, default_value_t = 10)]
        history: u32,
    },
}
#[derive(Debug, Subcommand)]
enum CheckCommand {
    Http(CreateHttpCheck),
    List { service_id: Uuid },
    Enable { check_id: Uuid },
    Disable { check_id: Uuid },
}
#[derive(Debug, Args)]
struct CreateHttpCheck {
    service_id: Uuid,
    url: String,
    #[arg(long, default_value_t = 200)]
    expected_status: u16,
    #[arg(long, default_value_t = 30)]
    interval_seconds: u64,
    #[arg(long, default_value_t = 5_000)]
    timeout_ms: u64,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let app = HealthCheckApp::open(DatabaseConfig::new(cli.database)?).await?;
    match cli.command {
        Command::Project {
            command: ProjectCommand::Create { name },
        } => {
            let project = app.create_project(name).await?;
            println!("Created project {} ({})", project.name, project.id);
        }
        Command::Project {
            command: ProjectCommand::List,
        } => {
            for project in app.list_projects().await? {
                println!("{}  {}", project.id, project.name);
            }
        }
        Command::Project {
            command: ProjectCommand::Summary { project_id },
        } => print_project(&app.project_summary(project_id).await?),
        Command::Service {
            command: ServiceCommand::Create { project_id, name },
        } => {
            let service = app.create_service(project_id, name).await?;
            println!("Created service {} ({})", service.name, service.id);
        }
        Command::Service {
            command: ServiceCommand::List { project_id },
        } => {
            for service in app.list_services(project_id).await? {
                println!("{}  {}", service.id, service.name);
            }
        }
        Command::Service {
            command:
                ServiceCommand::Health {
                    service_id,
                    history,
                },
        } => print_service(&app.service_health(service_id, history).await?),
        Command::Check {
            command: CheckCommand::Http(input),
        } => {
            let (check, initial) = app
                .create_http_check(
                    input.service_id,
                    input.url,
                    input.expected_status,
                    input.interval_seconds,
                    input.timeout_ms,
                )
                .await?;
            println!("Created enabled HTTP check {}", check.id);
            println!("Initial result: {}", format_result(&initial));
        }
        Command::Check {
            command: CheckCommand::List { service_id },
        } => {
            for check in app.list_checks(service_id).await? {
                print_check(&check, None);
            }
        }
        Command::Check {
            command: CheckCommand::Enable { check_id },
        } => {
            app.set_check_enabled(check_id, true).await?;
            println!("Enabled check {check_id}");
        }
        Command::Check {
            command: CheckCommand::Disable { check_id },
        } => {
            app.set_check_enabled(check_id, false).await?;
            println!("Disabled check {check_id}");
        }
        Command::Monitor => {
            println!("Monitoring enabled checks. Press Ctrl+C to stop.");
            app.monitor_until(async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("install Ctrl+C handler");
            })
            .await?;
            println!("Monitoring stopped.");
        }
    }
    Ok(())
}
fn print_project(summary: &ProjectSummary) {
    println!(
        "Project: {} ({}) [{}]",
        summary.project.name,
        summary.project.id,
        status_name(summary.status)
    );
    for service in &summary.services {
        println!(
            "  {} ({}) [{}]",
            service.service.name,
            service.service.id,
            status_name(service.status)
        );
    }
}
fn print_service(health: &ServiceHealth) {
    println!(
        "Service: {} ({}) [{}]",
        health.service.name,
        health.service.id,
        status_name(health.status)
    );
    for check in &health.checks {
        print_check(&check.check, check.latest_result.as_ref());
        for result in &check.recent_history {
            println!("    {}", format_result(result));
        }
    }
}
fn print_check(check: &healthcheck_core::Check, latest: Option<&CheckResult>) {
    let (url, expected_status) = match &check.kind {
        CheckKind::Http {
            url,
            expected_status,
            ..
        } => (url.as_str(), *expected_status),
        _ => ("unsupported", 0),
    };
    let state = if check.enabled { "enabled" } else { "disabled" };
    println!(
        "  HTTP {} ({}) [{}; every {}s; timeout {}ms; expects {}]",
        url, check.id, state, check.interval_seconds, check.timeout_ms, expected_status
    );
    if let Some(result) = latest {
        println!("    latest: {}", format_result(result));
    }
}
fn format_result(result: &CheckResult) -> String {
    let error = result
        .error
        .as_deref()
        .map(|error| format!(" — {error}"))
        .unwrap_or_default();
    format!(
        "{} at {} ({}ms){}",
        status_name(result.status),
        result.checked_at.to_rfc3339(),
        result.duration_ms,
        error
    )
}
fn status_name(status: HealthStatus) -> &'static str {
    match status {
        HealthStatus::Healthy => "HEALTHY",
        HealthStatus::Degraded => "DEGRADED",
        HealthStatus::Unhealthy => "UNHEALTHY",
        HealthStatus::Unknown => "UNKNOWN",
        _ => "UNKNOWN",
    }
}
