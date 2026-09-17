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
