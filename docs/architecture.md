# Architecture

This document will describe HealthCheck's server, agent, shared domain model, persistence layer, and dependency-aware incident evaluation as implementation begins.

## Core Domain

The `healthcheck-core` crate contains the initial infrastructure-independent domain model:

- `Project` groups related monitored services.
- `Service` represents one monitored component within a project.
- `Check` stores one configured health check for a service.
- `CheckKind` currently supports HTTP checks with primitive configuration only.
- `HealthStatus` represents interpreted health states.
- `CheckResult` records the generic result of one completed check execution.

The core crate intentionally does not depend on async runtimes, HTTP clients, web frameworks, database types, CLI frameworks, or transport-specific API types. Execution, scheduling, persistence, and protocol concerns remain outside the core domain.

## Check Execution

The `healthcheck-server` crate owns monitoring execution behavior. It currently provides a reusable `HttpCheckExecutor` that executes one configured HTTP `Check` and produces one core `CheckResult`.

HTTP result semantics are:

- `Healthy` means the request completed and returned the configured expected status.
- `Unhealthy` means the target was observed failing, including unexpected HTTP status, connection failure, DNS/TLS request failure, or timeout.
- `Unknown` is reserved for cases where the monitoring mechanism cannot interpret or execute the check itself, such as an unsupported future check kind.

## Scheduler Runtime

The server crate now includes an in-memory recurring scheduler:

```text
Configured Check
      |
      v
Scheduler
      |
      v
CheckExecutor
      |
      v
CheckResult
      |
      v
ResultSink
```

Scheduling uses fixed-delay semantics: a check's next run becomes due after the previous execution completes plus the configured interval. This avoids catch-up bursts and naturally prevents overlapping executions of the same check. Different checks may execute concurrently.

The scheduler keeps runtime state in memory after startup. Completed results are recorded through a `ResultSink`; `InMemoryResultSink` is available for tests and early runtime validation.

## Persistence

SQLite is the MVP persistence backend. SQLx migrations under `migrations/` are the schema source of truth for projects, services, checks, and check results.

```text
SQLite
  |
  v
Repositories
  |
  v
Load enabled checks
  |
  v
Scheduler
  |
  v
CheckExecutor
  |
  v
CheckResult
  |
  v
SqliteResultSink
  |
  v
SQLite
```

Domain models remain database-independent: `healthcheck-core` does not depend on SQLx or SQLite types. Persistence modules map database rows to core domain values and fail explicitly on invalid UUIDs, timestamps, enum values, or domain invariants.

Checks are loaded from SQLite at startup and registered with the in-memory scheduler. The scheduler does not poll the database on each tick; dynamic reload can be added later. Result persistence is integrated through `SqliteResultSink`, preserving the same `ResultSink` boundary used by `InMemoryResultSink`.

UUIDs are stored as canonical text. Timestamps are stored as UTC RFC3339 strings. Enums use explicit stable strings such as `HTTP`, `HEALTHY`, `DEGRADED`, `UNHEALTHY`, and `UNKNOWN`; these formats are compatibility-sensitive.

The current schema stores HTTP-specific check fields (`url`, `expected_status`) directly on `checks` because HTTP is the only implemented check kind. PostgreSQL, REST APIs, local agents, distributed protocol transport, and hot reload of database changes are still future work.

## Local CLI Product Slice

`healthcheck-server` is also the local CLI executable. Its application layer opens the configured SQLite path, validates and applies migrations, and composes existing repositories, the HTTP executor, scheduler, and SQLite result sink. The CLI itself only parses commands and renders human-readable reports.

Creating an HTTP check persists the enabled check and immediately executes it through `HttpCheckExecutor`; the first result is saved before the command returns. `monitor` loads enabled checks from SQLite at startup and passes them to the existing fixed-delay scheduler. Ctrl+C resolves the scheduler shutdown future, allowing active executions to finish and results to persist before exit.

Reporting is a separate server read-model layer. It combines repository reads into project summaries, service health, latest result per check, recent history, and enabled state. A service is `UNKNOWN` if it has no enabled checks or an enabled check has no result; an unhealthy enabled check makes the service unhealthy. Project status aggregates its service statuses using the same severity ordering.
