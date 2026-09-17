# HealthCheck

HealthCheck is planned as a distributed monitoring platform for remote services, local processes, and service dependency chains. The project is intended to explore reliable monitoring, agent/server communication, dependency-aware incident modeling, and root-cause-oriented health visualization.

This repository currently contains the initial project foundation, shared core domain types, one-shot HTTP check execution, an in-memory recurring scheduler, and SQLite-backed persistence. CLI, daemon, REST API, local agent, and distributed protocol surfaces have not been implemented yet.

## Goals

- Monitor remote services and local systems from a central server and deployable agents.
- Model dependency chains so downstream failures can be separated from likely root causes.
- Demonstrate Rust systems programming with asynchronous networking, persistence, and clear domain boundaries.
- Keep the architecture simple enough for a portfolio project while leaving room for production-realistic concerns.

## Planned Features

- One-shot remote HTTP checks.
- TCP checks.
- DNS checks.
- TLS and certificate checks.
- Local process and service monitoring.
- CPU, memory, and process metrics from local agents.
- In-memory recurring checks with configurable polling intervals.
- SQLite-backed storage for projects, services, checks, and check results.
- Health history and incident tracking.
- Dependency graphs between services and checks.
- Dependency-aware failure visualization.
- Server-sent events or WebSockets for live updates where appropriate.
- Optional web dashboard after the core backend is established.

## Architecture

The intended architecture is a central server that stores checks, receives observations, evaluates health state, and exposes APIs. Lightweight agents will run near monitored workloads and report local process, service, and system metrics. Shared domain logic belongs in `core`, while communication contracts belong in `protocol`.

```mermaid
flowchart LR
    Agent[Local Agent] -->|observations| Server[HealthCheck Server]
    Server --> Database[(SQLite)]
    Server --> Api[HTTP API / Live Events]
    Api --> Dashboard[Optional Dashboard]
    Core[core crate] -. shared domain logic .- Server
    Core -. shared domain logic .- Agent
    Protocol[protocol crate] -. shared contracts .- Server
    Protocol -. shared contracts .- Agent
```

## Repository Structure

- `crates/server` - SQLite persistence, in-memory scheduling, one-shot HTTP check execution, result sink abstractions, and planned central API and incident evaluation service.
- `crates/agent` - planned lightweight deployable monitor for local processes and system metrics.
- `crates/core` - shared domain types for projects, services, checks, health status, and check results.
- `crates/protocol` - planned server/agent communication structures.
- `migrations` - SQLx SQLite database migrations.
- `config/examples` - example configuration files once configuration is introduced.
- `docs` - architecture and protocol notes.
- `scripts` - future developer and operational scripts.
- `tests` - integration and system-level tests.

## Technology Stack

- Rust - primary language.
- Tokio - asynchronous runtime.
- Axum - planned HTTP server framework.
- SQLx - database access layer.
- SQLite - MVP persistence layer.
- Server-sent events or WebSockets - planned live event transport.

Only the Rust workspace structure, initial core domain model, one-shot HTTP check execution, in-memory recurring scheduler, and SQLite persistence are currently present.

## Development

Setup instructions will be expanded as implementation begins. No dependencies beyond the initial Rust workspace skeleton are required yet.

## Roadmap

- [ ] Define core health-check and incident domain models. Initial health-check domain types are in place; incident modeling is still pending.
- [x] Establish SQLite persistence migrations and repositories.
- [x] Implement one-shot remote HTTP check execution.
- [x] Implement repeated scheduling of enabled checks.
- [ ] Build CLI/application layer for creating resources, starting monitoring, and reporting persisted health state.
- [ ] Add agent registration and observation ingestion.
- [ ] Add local process and system monitoring.
- [ ] Model service dependencies and incident propagation.
- [ ] Add live health updates for clients.
- [ ] Expand integration and failure-mode tests.

## License

MIT. See `LICENSE`.

HealthCheck is under active development.
