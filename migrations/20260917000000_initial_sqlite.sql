PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS services (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_services_project_id
    ON services(project_id);

CREATE TABLE IF NOT EXISTS checks (
    id TEXT PRIMARY KEY NOT NULL,
    service_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    url TEXT NOT NULL,
    expected_status INTEGER NOT NULL,
    interval_seconds INTEGER NOT NULL,
    timeout_ms INTEGER NOT NULL,
    enabled INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE,
    CHECK (kind IN ('HTTP')),
    CHECK (expected_status BETWEEN 100 AND 599),
    CHECK (interval_seconds > 0),
    CHECK (timeout_ms > 0),
    CHECK (enabled IN (0, 1))
);

CREATE INDEX IF NOT EXISTS idx_checks_service_id
    ON checks(service_id);

CREATE INDEX IF NOT EXISTS idx_checks_enabled
    ON checks(enabled);

CREATE TABLE IF NOT EXISTS check_results (
    id TEXT PRIMARY KEY NOT NULL,
    check_id TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    status TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    error TEXT,
    FOREIGN KEY (check_id) REFERENCES checks(id) ON DELETE CASCADE,
    CHECK (status IN ('HEALTHY', 'DEGRADED', 'UNHEALTHY', 'UNKNOWN')),
    CHECK (duration_ms >= 0)
);

CREATE INDEX IF NOT EXISTS idx_check_results_check_id
    ON check_results(check_id);

CREATE INDEX IF NOT EXISTS idx_check_results_checked_at
    ON check_results(checked_at);

CREATE INDEX IF NOT EXISTS idx_check_results_check_id_checked_at
    ON check_results(check_id, checked_at);
