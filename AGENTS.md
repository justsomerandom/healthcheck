# AGENTS.md

## Project Purpose

HealthCheck is intended to become a distributed monitoring platform for remote services, local processes, and dependency-aware incident analysis.

## Engineering Priorities

- Correctness in health-state evaluation and incident modeling.
- Clear boundaries between server, agent, shared domain logic, and wire protocol.
- Observability for checks, scheduling, agent communication, and failures.
- Security around agent communication, configuration, and secrets.
- Testability of monitoring behavior without requiring live infrastructure.
- Minimal unnecessary dependencies.

## Architecture Rules

- Keep shared domain logic in `crates/core`.
- Keep server/agent communication contracts in `crates/protocol`.
- Keep server-specific concerns in `crates/server`.
- Keep local host inspection and reporting in `crates/agent`.
- Do not let server persistence details leak into agent code.
- Treat dependency graph evaluation as domain logic, not UI logic.
- Document protocol changes before relying on them across crates.

## Coding Guidelines

- Use idiomatic Rust with explicit error handling.
- Prefer small modules organized around domain responsibilities.
- Do not introduce unnecessary abstractions.
- Do not silently change architecture or crate boundaries.
- Do not add technologies merely for resume value.
- Prefer well-maintained libraries with clear maintenance histories.
- Preserve backwards compatibility once public APIs or protocol versions exist.
- Validate external input from APIs, agents, and configuration files.
- Keep secrets out of the repository.
- Avoid generated code unless justified and documented.
- Add tests with meaningful behavior changes.
- Document non-obvious design decisions.

## Testing

Future tests should cover core state transitions, check scheduling behavior, protocol compatibility, API contracts, persistence boundaries, and agent/server integration paths. Unit tests should live near domain code; integration tests should focus on realistic monitoring flows.

## Documentation

Update `README.md` and `docs/` when architecture, protocol behavior, configuration, or user-visible behavior changes.

## Agent Workflow

Before making significant changes:

1. Inspect the existing architecture.
2. Understand relevant domain code.
3. Make the smallest coherent change.
4. Run relevant formatting, linting, and tests.
5. Summarize architectural consequences.

`AGENTS.md` may be expanded as this project matures.
