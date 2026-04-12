-- T011: initial schema for the cross-repo workbench.
-- Implements data-model.md §2 (all 9 persistent entities) plus the SessionOwnership
-- constraint from spec-panel round 1 item #2.
--
-- Design notes:
--   * All timestamps are ISO-8601 UTC text.
--   * Soft deletion uses `archived_at` / `ended_at`; rows are never DELETEd except
--     explicitly by user action.
--   * Partial unique index on (canonical_path) WHERE archived_at IS NULL enforces
--     "no two active projects with the same root" per invariant #2.
--   * CHECK constraints enforce enum values for status / status_source / ownership
--     and reminder state.

PRAGMA foreign_keys = ON;

----------------------------------------------------------------------
-- 2.1 projects
----------------------------------------------------------------------
CREATE TABLE projects (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_path  TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    added_at        TEXT NOT NULL,
    last_active_at  TEXT NULL,
    archived_at     TEXT NULL,
    settings_json   TEXT NOT NULL DEFAULT '{}'
);

CREATE UNIQUE INDEX idx_projects_canonical_path_active
    ON projects(canonical_path)
    WHERE archived_at IS NULL;

CREATE INDEX idx_projects_archived_at ON projects(archived_at);

----------------------------------------------------------------------
-- 2.2 sessions
----------------------------------------------------------------------
CREATE TABLE sessions (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id          INTEGER NOT NULL REFERENCES projects(id),
    label               TEXT NOT NULL,
    pid                 INTEGER NULL,
    status              TEXT NOT NULL
        CHECK (status IN ('working','idle','needs_input','ended','error')),
    status_source       TEXT NOT NULL
        CHECK (status_source IN ('ipc','heuristic')),
    ownership           TEXT NOT NULL
        CHECK (ownership IN ('workbench','wrapper')),
    started_at          TEXT NOT NULL,
    ended_at            TEXT NULL,
    last_activity_at    TEXT NOT NULL,
    last_heartbeat_at   TEXT NULL,
    metadata_json       TEXT NOT NULL DEFAULT '{}',
    working_directory   TEXT NOT NULL,
    exit_code           INTEGER NULL,
    error_reason        TEXT NULL
);

CREATE INDEX idx_sessions_project_status ON sessions(project_id, status);
CREATE INDEX idx_sessions_last_activity ON sessions(last_activity_at DESC);
CREATE INDEX idx_sessions_ended_at ON sessions(ended_at);

----------------------------------------------------------------------
-- 2.3 companion_terminals
----------------------------------------------------------------------
CREATE TABLE companion_terminals (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   INTEGER NOT NULL REFERENCES sessions(id),
    pid          INTEGER NULL,
    shell_path   TEXT NOT NULL,
    initial_cwd  TEXT NOT NULL,
    started_at   TEXT NOT NULL,
    ended_at     TEXT NULL
);

CREATE UNIQUE INDEX idx_companion_terminals_session_id ON companion_terminals(session_id);

----------------------------------------------------------------------
-- 2.4 notes
----------------------------------------------------------------------
CREATE TABLE notes (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id),
    content     TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_notes_project_created ON notes(project_id, created_at DESC);

----------------------------------------------------------------------
-- 2.5 reminders
----------------------------------------------------------------------
CREATE TABLE reminders (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id),
    content     TEXT NOT NULL,
    state       TEXT NOT NULL CHECK (state IN ('open','done')),
    created_at  TEXT NOT NULL,
    done_at     TEXT NULL
);

CREATE INDEX idx_reminders_state_created ON reminders(state, created_at);
CREATE INDEX idx_reminders_project_state_created
    ON reminders(project_id, state, created_at DESC);

----------------------------------------------------------------------
-- 2.6 workspace_state (single row)
----------------------------------------------------------------------
CREATE TABLE workspace_state (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    payload_json  TEXT NOT NULL,
    saved_at      TEXT NOT NULL
);

----------------------------------------------------------------------
-- 2.7 layouts
----------------------------------------------------------------------
CREATE TABLE layouts (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL UNIQUE,
    payload_json  TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

----------------------------------------------------------------------
-- 2.8 notification_preferences
----------------------------------------------------------------------
CREATE TABLE notification_preferences (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id     INTEGER NULL REFERENCES projects(id),
    channels_json  TEXT NOT NULL,
    quiet_hours    TEXT NULL,
    updated_at     TEXT NOT NULL
);

-- Per-project uniqueness; global row is the single NULL-project_id row and
-- its uniqueness is enforced in Rust (SQLite does not treat NULL values as
-- equal in UNIQUE indexes by default).
CREATE UNIQUE INDEX idx_notification_preferences_project_id
    ON notification_preferences(project_id)
    WHERE project_id IS NOT NULL;

----------------------------------------------------------------------
-- 2.9 alerts
----------------------------------------------------------------------
CREATE TABLE alerts (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id        INTEGER NOT NULL REFERENCES sessions(id),
    project_id        INTEGER NOT NULL REFERENCES projects(id),
    kind              TEXT NOT NULL CHECK (kind IN ('needs_input')),
    reason            TEXT NULL,
    raised_at         TEXT NOT NULL,
    acknowledged_at   TEXT NULL,
    cleared_by        TEXT NULL
        CHECK (cleared_by IN ('user','session_resumed','session_ended'))
);

CREATE INDEX idx_alerts_session_open ON alerts(session_id, acknowledged_at);
CREATE INDEX idx_alerts_open ON alerts(acknowledged_at);
