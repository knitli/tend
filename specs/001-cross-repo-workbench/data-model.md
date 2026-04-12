# Phase 1 Data Model: Cross-Repo Agent Workbench

**Feature**: `001-cross-repo-workbench`
**Date**: 2026-04-11
**Status**: Draft (Phase 1)
**Depends on**: `spec.md`, `research.md`

This document defines the persistent and in-memory data model for the workbench. Persistent entities map to SQLite tables under `src-tauri/migrations/`; in-memory-only entities are marked as such.

---

## 1. Entity Relationship Overview

```text
                         ┌────────────────┐
                         │    Project     │───────┐
                         └────────────────┘       │
                               │ 1                │ 1
                               │                  │
                               │ *                │ *
                ┌──────────────┴────┐      ┌──────┴───────┐
                │     Session      │      │  Scratchpad  │
                └──────────────────┘      │   (1:1 w/    │
                       │ 1                │    Project)  │
                       │ 1                └──────────────┘
                       ▼                         │ 1
                ┌────────────────┐                │ *
                │    Companion   │       ┌────────┴────────┐
                │    Terminal    │       │     Note        │
                └────────────────┘       └─────────────────┘
                       │                         │
                       │                 ┌───────┴─────────┐
                       │ emits           │     Reminder    │
                       ▼                 └─────────────────┘
                ┌────────────────┐
                │     Alert      │
                └────────────────┘

            ┌──────────────────┐         ┌──────────────────┐
            │  Workspace State │         │     Layout       │
            │   (single row)   │         │  (user-named)    │
            └──────────────────┘         └──────────────────┘

            ┌──────────────────────┐
            │ Notification         │
            │ Preference           │  global row + per-project rows
            └──────────────────────┘
```

Relationships:

- `Project 1——* Session`
- `Project 1——1 Scratchpad` (implicit; no separate row — Notes and Reminders reference Project directly)
- `Project 1——* Note`
- `Project 1——* Reminder`
- `Session 1——1 CompanionTerminal` (0..1 until first activation, then 1)
- `Session 1——* Alert`
- `Project 1——* NotificationPreference` (plus a global-default row with `project_id = NULL`)
- `Layout 1——* LayoutSlot` (not shown above; internal structure inside `layouts.payload_json`)

---

## 2. Persistent Entities (SQLite)

All tables use `INTEGER PRIMARY KEY` surrogate ids (`rowid` aliased as `id`) and store timestamps as ISO-8601 text for human-readability in the db file. `archived_at` columns are used for soft-deletion.

### 2.1 `projects`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Surrogate id |
| `canonical_path` | TEXT NOT NULL | `std::fs::canonicalize` result; stable primary identity |
| `display_name` | TEXT NOT NULL | User-editable label |
| `added_at` | TEXT NOT NULL | ISO-8601 UTC |
| `last_active_at` | TEXT NULL | Updated when any session in this project has activity |
| `archived_at` | TEXT NULL | Set when user removes project; child rows follow |
| `settings_json` | TEXT NOT NULL DEFAULT '{}' | Per-project freeform settings |

Indexes:

- `UNIQUE(canonical_path) WHERE archived_at IS NULL` — no two active projects with the same root
- `INDEX(archived_at)` — quick filters for "active only" and "archived only" views

Validation (enforced in Rust, not SQL):

- `canonical_path` must be absolute and resolve successfully at registration time
- `display_name` must be non-empty after trim; defaults to basename of `canonical_path`

---

### 2.2 `sessions`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | Surrogate id |
| `project_id` | INTEGER NOT NULL REFERENCES projects(id) | Owning project |
| `label` | TEXT NOT NULL | User-visible label (e.g. `"parser rewrite"`, defaults to `"session N"`) |
| `pid` | INTEGER NULL | OS pid of the agent child process (null before spawn, null after exit) |
| `status` | TEXT NOT NULL | Enum: `working` \| `idle` \| `needs_input` \| `ended` \| `error` |
| `status_source` | TEXT NOT NULL | Enum: `ipc` \| `heuristic` — tracks whether the status came from cooperative IPC or fallback detection |
| `ownership` | TEXT NOT NULL | Enum: `workbench` \| `wrapper` — who owns the PTY master end; determines whether `session_send_input` / `session_resize` / `session_end` are accepted (see spec FR-008 and research.md §6) |
| `started_at` | TEXT NOT NULL | ISO-8601 UTC |
| `ended_at` | TEXT NULL | Set on exit |
| `last_activity_at` | TEXT NOT NULL | Updated on every PTY output byte |
| `metadata_json` | TEXT NOT NULL DEFAULT '{}' | Agent-provided metadata (task title, branch, model, etc.) |
| `working_directory` | TEXT NOT NULL | Actual session cwd (worktree / submodule aware; may differ from project root) |

Indexes:

- `INDEX(project_id, status)` — primary overview query
- `INDEX(last_activity_at DESC)` — activity summary sorts

State transitions:

```text
                    spawn
         ╔═════════════════════╗
         ║        working      ║
         ╚══════════╤══════════╝
               │    │
               │    │ no output ≥ 5s
               │    ▼
               │  ┌─────────┐        output
               │  │  idle   │─────────────────┐
               │  └────┬────┘                 │
               │       │ prompt-pattern /     │
               │       │ IPC needs_input      │
               │       ▼                      │
               │  ┌──────────────┐            │
               │  │ needs_input  │────────────┘
               │  └──────┬───────┘
               │         │ user response / agent acknowledges
               │         ▼
               │     working (loop)
               │
               │ child exit 0      child exit != 0 or panic
               ▼                          │
         ┌─────────┐                      ▼
         │  ended  │                 ┌────────┐
         └─────────┘                 │ error  │
                                     └────────┘
```

Validation:

- `status ∈ {working, idle, needs_input, ended, error}` — enforced in Rust, CHECK constraint in SQL
- `status_source ∈ {ipc, heuristic}`
- `ownership ∈ {workbench, wrapper}` — CHECK constraint; immutable after row creation
- `pid` must be set whenever `status ∈ {working, idle, needs_input}` and null when `ended`/`error`
- `ended_at` must be set iff `status ∈ {ended, error}`
- `ownership = wrapper` implies the backend MUST reject `session_send_input`, `session_resize`, and `session_end` with `SESSION_READ_ONLY` — the wrapper process owns the PTY and the user inputs via the launching terminal

Lifecycle / persistence rules:

- A session row is created immediately on spawn and survives the session's lifetime; it is **not** deleted when the session ends. Ended sessions are retained as history and can be pruned by a background sweep at a retention threshold (default: 7 days).
- Crash recovery: on workbench startup, any row with `status ∈ {working, idle, needs_input}` but whose `pid` is no longer running is transitioned to `ended` with a `status_source = heuristic` note.

---

### 2.3 `companion_terminals`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `session_id` | INTEGER NOT NULL REFERENCES sessions(id) | 1:1 with Session |
| `pid` | INTEGER NULL | Null when not currently spawned |
| `shell_path` | TEXT NOT NULL | Resolved shell (respects `$SHELL`, falls back to `/bin/sh`) |
| `initial_cwd` | TEXT NOT NULL | Session's working directory at pairing time |
| `started_at` | TEXT NOT NULL | Most recent spawn time |
| `ended_at` | TEXT NULL | Previous spawn's exit time; cleared on respawn |

Indexes:

- `UNIQUE(session_id)` — strict 1:1

Lifecycle:

- Created lazily on first activation of the session (not on session spawn).
- Reused on subsequent activations as long as the underlying PTY is alive (`pid` still exists).
- Automatically recreated transparently if the user kills it (spec FR-016).
- Scratch buffer for companion output is kept in memory only (not persisted) — if the user cares about a transcript, that's a future feature.

Validation:

- `initial_cwd` must be an existing directory at spawn time (otherwise spawn fails and an alert is raised)

---

### 2.4 `notes`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `project_id` | INTEGER NOT NULL REFERENCES projects(id) | |
| `content` | TEXT NOT NULL | Plain text (light markdown rendered by UI) |
| `created_at` | TEXT NOT NULL | ISO-8601 UTC |
| `updated_at` | TEXT NOT NULL | Updated on edit |

Indexes:

- `INDEX(project_id, created_at DESC)` — chronological per-project view

Validation:

- `content` trimmed; may not be empty (empty deletes the row instead)
- `updated_at ≥ created_at`

---

### 2.5 `reminders`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `project_id` | INTEGER NOT NULL REFERENCES projects(id) | |
| `content` | TEXT NOT NULL | |
| `state` | TEXT NOT NULL | Enum: `open` \| `done` |
| `created_at` | TEXT NOT NULL | Used to compute age indicator (FR-031) |
| `done_at` | TEXT NULL | Set when state flips to `done` |

Indexes:

- `INDEX(state, created_at)` — primary query for cross-project overview
- `INDEX(project_id, state, created_at DESC)` — per-project view

Validation:

- `state ∈ {open, done}` — CHECK constraint
- `done_at` set iff `state = done`

Query helpers:

- `SELECT * FROM reminders WHERE state = 'open' ORDER BY project_id, created_at DESC` drives the Cross-Project Overview grouping (FR-028).

---

### 2.6 `workspace_state`

Single-row table (`CHECK(id = 1)`) storing the last-known workspace so the workbench auto-restores on launch.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK CHECK(id = 1) | Enforces single-row |
| `payload_json` | TEXT NOT NULL | Serialized `WorkspaceState` (see §3) |
| `saved_at` | TEXT NOT NULL | |

Payload structure (JSON):

```json
{
  "version": 1,
  "active_project_ids": [1, 4],
  "focused_session_id": 12,
  "pane_layout": "split",
  "ui": { "sidebar_width": 280, "scratchpad_visible": true }
}
```

Write cadence:

- On graceful shutdown
- Every 30 s of UI activity (debounced)
- On major UI state changes (project add/remove, split pane toggle, activation)

---

### 2.7 `layouts`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `name` | TEXT NOT NULL UNIQUE | User-supplied |
| `payload_json` | TEXT NOT NULL | Same schema as `workspace_state.payload_json` with additional `project_ids` list of projects the layout *references* |
| `created_at` | TEXT NOT NULL | |
| `updated_at` | TEXT NOT NULL | |

Indexes:

- `UNIQUE(name)`

---

### 2.8 `notification_preferences`

One row per `(project_id)` with `project_id = NULL` representing the global default. Lookups fall back from project → global.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `project_id` | INTEGER NULL REFERENCES projects(id) | NULL = global default |
| `channels_json` | TEXT NOT NULL | JSON list, e.g. `["in_app", "os_notification"]` |
| `quiet_hours` | TEXT NULL | JSON object: `{ "start": "22:00", "end": "07:00", "timezone": "local" }` |
| `updated_at` | TEXT NOT NULL | |

Indexes:

- `UNIQUE(project_id)` (with NULL allowed exactly once — enforced in Rust, not SQL — or split into a separate `global_notification_preference` table to avoid the NULL-uniqueness ambiguity)

Validation:

- `channels_json` must contain at least one of `in_app` \| `os_notification` \| `terminal_bell` \| `silent`
- `silent` implies no OS or terminal output but in-app alerts still show (FR-013 quiet-hours semantics)

---

### 2.9 `alerts`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PK | |
| `session_id` | INTEGER NOT NULL REFERENCES sessions(id) | |
| `project_id` | INTEGER NOT NULL REFERENCES projects(id) | Denormalized for overview queries |
| `kind` | TEXT NOT NULL | Enum: `needs_input` (v1 has only this; left open for future kinds) |
| `reason` | TEXT NULL | Optional human-readable reason from IPC |
| `raised_at` | TEXT NOT NULL | |
| `acknowledged_at` | TEXT NULL | Set when user clears it OR session auto-clears by resuming work |
| `cleared_by` | TEXT NULL | Enum: `user` \| `session_resumed` \| `session_ended` — NULL while open |

Indexes:

- `INDEX(session_id, acknowledged_at)` — "current open alert for session"
- `INDEX(acknowledged_at)` — "list all active alerts"

Lifecycle:

- Exactly one open alert per session per kind at a time. A second `needs_input` transition for a session whose previous `needs_input` alert is still open is **coalesced** (same row, new `raised_at`) to avoid alert storms.

---

## 3. In-Memory Domain Types (Rust, `src-tauri/src/model/`)

These types are the Rust representations of the tables above, plus runtime-only structures not persisted to disk.

### 3.1 Persisted types (mirror tables)

```rust
pub struct Project {
    pub id: ProjectId,
    pub canonical_path: PathBuf,
    pub display_name: String,
    pub added_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub settings: ProjectSettings,
}

pub struct Session {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub label: String,
    pub pid: Option<Pid>,
    pub status: SessionStatus,
    pub status_source: StatusSource,
    pub ownership: SessionOwnership,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub last_activity_at: DateTime<Utc>,
    pub metadata: SessionMetadata,
    pub working_directory: PathBuf,
}

pub enum SessionStatus { Working, Idle, NeedsInput, Ended, Error }
pub enum StatusSource { Ipc, Heuristic }
pub enum SessionOwnership { Workbench, Wrapper }

pub struct CompanionTerminal { /* … */ }
pub struct Note { /* … */ }
pub struct Reminder { /* … */ }
pub struct Layout { /* … */ }
pub struct NotificationPreference { /* … */ }
pub struct Alert { /* … */ }
```

Newtype ids prevent accidental cross-entity id mixups:

```rust
pub struct ProjectId(i64);
pub struct SessionId(i64);
pub struct CompanionTerminalId(i64);
pub struct NoteId(i64);
pub struct ReminderId(i64);
pub struct LayoutId(i64);
pub struct AlertId(i64);
```

### 3.2 In-memory-only types (runtime state)

**`LiveSession`** — the actor handle for an active session. For workbench-owned sessions in their first lifetime, owns the PTY master end; for wrapper-owned sessions and for workbench-owned sessions reattached after a workbench restart, runs in **attached-mirror** mode with no PTY ownership (the `pty` field is `None`, writes are rejected by the backend with `SESSION_READ_ONLY`, and the handle exists only to broadcast session events to subscribers). The `reattached_mirror: bool` flag distinguishes the post-restart workbench-owned mirror from a live workbench-owned handle — see `research.md §6` restart caveat and `tasks.md T025` for the reattach pass.

```rust
pub struct LiveSession {
    pub record: Session,
    pub pty: Option<Arc<dyn Pty>>, // None for wrapper-owned and reattached-mirror
    pub reattached_mirror: bool,    // true iff this handle was created by `reconcile_and_reattach`
    pub output_ring: ArrayQueue<OutputFrame>, // last 200 lines
    pub events: broadcast::Sender<SessionEvent>,
    pub supervisor: JoinHandle<()>,
}

pub enum SessionEvent {
    OutputChunk(Vec<u8>),
    StatusChanged { from: SessionStatus, to: SessionStatus, source: StatusSource },
    AlertRaised(Alert),
    AlertCleared { alert_id: AlertId, by: AlertClearedBy },
    Ended { code: Option<i32> },
}
```

**`WorkspaceState`** — in-memory mirror of `workspace_state.payload_json`, updated continuously and flushed by the debounced writer.

**`ActivitySummary`** — derived, not stored. Computed on demand from `LiveSession.output_ring` and `Session.metadata`.

**`CrossProjectOverview`** — derived view of "all open reminders grouped by project," computed from a single SQL query and streamed to the frontend as a reactive store.

---

## 4. Key Invariants

These invariants are maintained by the backend and tested in integration tests:

1. **Every live session has exactly one row in `sessions` and exactly zero or one rows in `companion_terminals`.** (0 before first activation, 1 after.)
2. **No two active projects share the same `canonical_path`.** Archived projects may collide with active ones (re-adding an archived project reactivates it via `canonical_path` match).
3. **An `ended` or `error` session never has a non-null `pid`.** Startup crash-recovery enforces this on every launch.
4. **A `needs_input` alert is open iff its session is in status `needs_input`.** Resume/end transitions auto-clear matching open alerts.
5. **A project's scratchpad survives every lifecycle event except explicit deletion from an archived project.** Neither session end nor project archival removes notes or reminders.
6. **`workspace_state` has exactly one row.** Enforced by `CHECK(id = 1)` and reconciled on startup.
7. **Companion terminal's initial cwd is always the session's `working_directory` at the time of pairing**, not the project root. Respects worktrees and submodules.
8. **Every session's `last_activity_at` is monotonically non-decreasing.** Clock adjustments are clamped, not reversed.
9. **Wrapper-owned sessions are read-only from the workbench.** `session_send_input`, `session_resize`, and `session_end` MUST return `SESSION_READ_ONLY` for any session with `ownership = wrapper`. The wrapper process owns the PTY master and the user inputs via the launching terminal. `ownership` is set at row creation and is immutable.

---

## 5. Migration Plan

Initial migration `20260411000001_init.sql` creates all nine tables plus CHECK constraints. Subsequent schema changes follow the standard `sqlx migrate add <name>` workflow with forward-only migrations. Rollback is not supported for v1 — the workbench's state is local and reproducible, so rollback is solved by "wipe the DB and re-register projects."

---

## 6. Open Questions (non-blocking)

These are not blockers for Phase 1 but should be decided during implementation:

- **Pruning policy for ended sessions**: 7-day retention is a default; is that configurable per-project? (Leaning yes via `projects.settings_json`.)
- **Alert deduplication window**: should two `needs_input` transitions within 1 second be merged? (Leaning yes.)
- **Session label collision within a project**: enforce uniqueness or auto-suffix? (Leaning auto-suffix: `"parser rewrite (2)"`.)
- **Scratchpad size soft limit**: at what note/reminder count do we switch to paginated loading in the UI? (Leaning 500 entries per project.)

These live in implementation tasks, not in the spec or plan.
