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

The current scheduler keeps configuration and runtime state in memory. Completed results are recorded through a `ResultSink`; `InMemoryResultSink` is available for tests and early runtime validation. Persistent storage, APIs, local agents, and distributed protocol transport are still future work.
