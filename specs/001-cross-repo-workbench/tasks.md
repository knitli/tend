# Tasks: Cross-Repo Agent Workbench

**Feature**: `001-cross-repo-workbench`
**Input**: Design documents from `/specs/001-cross-repo-workbench/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. TDD is mandated by the plan (Constitution Check, contracts README) — every Tauri command and daemon IPC verb has a contract test written **before** its implementation.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Parallelizable (different files, no dependency on an incomplete task)
- **[Story]**: `US1`..`US6` — only on user-story phases
- Paths are absolute under `/home/knitli/agentui/`

## Path Conventions

- **Backend crate**: `src-tauri/` (Rust)
- **Frontend**: `src/` (Svelte 5 + Vite)
- **CLI wrapper crate**: `cli/` (Rust, standalone)
- **Cross-crate integration + E2E tests**: `tests/`
- **Per-crate unit/contract tests**: inside each crate (`src-tauri/tests/`, `cli/tests/`)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Cargo workspace, Tauri bootstrap, frontend scaffold, toolchain baselines.

- [ ] T001 Create Cargo workspace root `Cargo.toml` at `/home/knitli/agentui/Cargo.toml` with members `["src-tauri", "cli"]` and shared `[workspace.dependencies]` for `tokio`, `sqlx`, `serde`, `serde_json`, `tracing`, `thiserror`, `anyhow`
- [ ] T002 Create `src-tauri/Cargo.toml` with Tauri 2.x, `portable-pty`, `sqlx` (sqlite, runtime-tokio-rustls, macros), `tokio` (full), `notify`, `sysinfo`, `tracing`, `tracing-subscriber`, `serde`, `serde_json`, `thiserror`, `anyhow`, `dirs` (for XDG), `tauri-plugin-notification`
- [ ] T003 Create `cli/Cargo.toml` with `clap` (derive), `portable-pty`, `tokio` (rt, io-util, net, macros), `serde`, `serde_json`, `anyhow`, `dirs`
- [ ] T004 [P] Create `src-tauri/tauri.conf.json` with window defaults (1400×900, resizable, title "agentui"), allowlist restricted to `invoke` only, productName `agentui`, identifier `io.knitli.agentui`
- [ ] T005 [P] Create `src-tauri/build.rs` invoking `tauri_build::build()`
- [ ] T006 [P] Create `package.json` at `/home/knitli/agentui/package.json` with Vite, Svelte 5, TypeScript, xterm.js 5, xterm-addon-fit, xterm-addon-web-links, marked, DOMPurify, @tauri-apps/api, @tauri-apps/cli; scripts `dev`, `build`, `tauri`, `test`, `e2e`
- [ ] T007 [P] Create `pnpm-workspace.yaml`, `vite.config.ts` (Svelte plugin, Tauri-aware clearScreen/server.port/envPrefix), `svelte.config.js`, `tsconfig.json` (strict, bundler module resolution)
- [ ] T008 [P] Create `src/app.html`, `src/app.css` (reset + theme tokens), `src/main.ts` (mount Svelte app), `src/routes/+page.svelte` (empty shell)
- [ ] T009 [P] Create `rustfmt.toml` (edition 2021, max_width 100), `.clippy.toml`, workspace `.editorconfig`
- [ ] T010 Verify `pnpm install && cargo check && pnpm tauri dev` opens an empty window; document in `specs/001-cross-repo-workbench/quickstart.md` if any system-dep gotchas appear

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Everything every user story depends on — database schema, error types, domain ID newtypes, PTY wrapper, Tauri bootstrap, test infrastructure, daemon-IPC transport layer.

**⚠️ CRITICAL**: No user story work may begin until this phase is complete.

### Data model & error boundaries

- [ ] T011 Create `src-tauri/migrations/20260411000001_init.sql` containing all 9 tables from `data-model.md`: `projects`, `sessions`, `companion_terminals`, `notes`, `reminders`, `workspace_state` (single-row), `layouts`, `notification_preferences`, `alerts` — with all indexes, foreign keys, and CHECK constraints from the data model
- [ ] T012 Create `src-tauri/src/model/mod.rs` defining Rust newtype IDs (`ProjectId`, `SessionId`, `CompanionId`, `NoteId`, `ReminderId`, `LayoutId`, `AlertId`, `NotificationPreferenceId`) wrapping `i64`, each `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]`
- [ ] T013 [P] Create `src-tauri/src/model/project.rs` with `Project`, `ProjectSettings`, status enums; `src-tauri/src/model/session.rs` with `Session`, `SessionStatus` (`Working`/`Idle`/`NeedsInput`/`Ended`/`Error`), `StatusSource` (`Cooperative`/`Heuristic`); `src-tauri/src/model/alert.rs`, `src-tauri/src/model/companion.rs`, `src-tauri/src/model/note.rs`, `src-tauri/src/model/reminder.rs`, `src-tauri/src/model/workspace.rs`, `src-tauri/src/model/layout.rs`, `src-tauri/src/model/notification.rs` — each with `Serialize`/`Deserialize` for Tauri command JSON boundary
- [ ] T014 Create `src-tauri/src/error.rs` defining `WorkbenchError { code, message, details }` and variant enum `ErrorCode` matching `contracts/tauri-commands.md` §8 catalog (`NOT_FOUND`, `ALREADY_EXISTS`, `ALREADY_REGISTERED`, `NOT_ARCHIVED`, `PATH_NOT_FOUND`, `PATH_NOT_A_DIRECTORY`, `WORKING_DIRECTORY_INVALID`, `PROJECT_ARCHIVED`, `SPAWN_FAILED`, `COMPANION_SPAWN_FAILED`, `WRITE_FAILED`, `SESSION_ENDED`, `CONTENT_EMPTY`, `NAME_TAKEN`, `INTERNAL`) plus daemon-IPC extras (`PROTOCOL_ERROR`, `MESSAGE_TOO_LARGE`, `UNAUTHORIZED`); implement `Serialize` for Tauri JSON error boundary and `From<sqlx::Error>`, `From<std::io::Error>` conversions

### Database layer

- [ ] T015 Create `src-tauri/src/db/mod.rs` with async `Database` type wrapping `sqlx::SqlitePool`, `Database::open(path)` that resolves XDG data dir via `dirs::data_local_dir().join("agentui/workbench.db")`, creates parent dir, opens pool with `SqlitePoolOptions`, and runs `sqlx::migrate!("./migrations")`
- [ ] T016 Create `src-tauri/src/db/queries.rs` with shared helpers `fetch_one_as`, `fetch_optional_as`, `fetch_all_as` converting `sqlx::Error` → `WorkbenchError`

### Workbench state & tauri bootstrap

- [ ] T017 Create `src-tauri/src/state.rs` with `WorkbenchState { db: Database, live_sessions: Arc<RwLock<HashMap<SessionId, LiveSessionHandle>>>, alert_bus: broadcast::Sender<Alert>, event_bus: broadcast::Sender<SessionEventEnvelope> }`, constructor `WorkbenchState::new(db) -> Self`, stub `LiveSessionHandle` type alias
- [ ] T018 Create `src-tauri/src/lib.rs` exposing `run()` that initializes `tracing_subscriber`, opens the DB, builds `WorkbenchState`, installs `.manage(state)`, registers an empty `tauri::generate_handler![]`, installs the notification plugin, and calls `tauri::Builder::run`
- [ ] T019 Create `src-tauri/src/main.rs` that calls `agentui_workbench::run()`

### PTY abstraction

- [ ] T020 Create `src-tauri/src/session/pty.rs` wrapping `portable-pty::native_pty_system()` with `Pty::spawn(command: &[String], cwd: &Path, env: &BTreeMap<String,String>) -> Result<Pty>`, `Pty::resize(cols, rows)`, `Pty::writer()`, `Pty::reader()` (thread-to-channel bridge into tokio mpsc), `Pty::pid()`, `Pty::wait() -> i32`; unit tests in same file under `#[cfg(test)]` covering spawn/echo/exit

### Daemon IPC transport skeleton (handlers come in US1)

- [ ] T021 Create `src-tauri/src/daemon/protocol.rs` with serde enums `Request` (`Hello`, `RegisterSession`, `UpdateStatus`, `EmitAlert`, `Heartbeat`, `EndSession`) and `Response` (`Welcome`, `SessionRegistered`, `Ack`, `Err`) matching `contracts/daemon-ipc.md` §3 field-for-field; `#[serde(tag = "kind", rename_all = "snake_case")]`
- [ ] T022 Create `src-tauri/src/daemon/server.rs` with `DaemonServer::bind(path)` that creates the Unix socket at `$XDG_RUNTIME_DIR/agentui.sock` (fallback `/tmp/agentui-$UID.sock`), chmod `0600`, accept-loop spawning per-connection tokio task; `read_frame(&mut reader) -> Result<Vec<u8>>` and `write_frame(&mut writer, bytes)` implementing little-endian u32 length-prefix, 64 KiB cap (return `MESSAGE_TOO_LARGE` on overflow); handler dispatch stub
- [ ] T023 Create `src-tauri/src/daemon/mod.rs` spawning the daemon server from `run()` alongside Tauri, with graceful shutdown on `ctrl_c`
- [ ] T024 [P] Create `src-tauri/src/daemon/handlers.rs` stub with `async fn dispatch(req: Request, state: &WorkbenchState) -> Response` returning `protocol_error("not yet implemented")` for every variant — real implementations in US1

### Crash recovery & tracing

- [ ] T025 Create `src-tauri/src/session/recovery.rs` with `reconcile_stale_sessions(db) -> Result<usize>` that updates any `sessions.status = 'working'` or `'needs_input'` row whose `ended_at IS NULL` to `status = 'ended', ended_at = now(), exit_code = NULL, error = 'workbench_restart'`; wired into `run()` after DB open, before Tauri start
- [ ] T026 [P] Configure `tracing_subscriber` in `lib.rs` with env-controlled filter (`AGENTUI_LOG`), JSON output in release, pretty in dev

### Test infrastructure

- [ ] T027 [P] Create `src-tauri/tests/common/mod.rs` with helpers: `temp_db()` returning an isolated in-memory SQLite + migrations, `temp_socket_path()` yielding a unique socket path under `std::env::temp_dir()`, `mock_state()` building a `WorkbenchState` on top of `temp_db()`
- [ ] T028 [P] Create `cli/tests/common/mod.rs` with a mock workbench IPC server that accepts a connection and responds to `hello` with `welcome`, for CLI happy-path tests

### Frontend foundation

- [ ] T029 [P] Create `src/lib/api/invoke.ts` wrapping `@tauri-apps/api/core` `invoke` with typed error conversion (`WorkbenchError` → JS `WorkbenchError` class with `code`, `message`, `details`); `src/lib/api/events.ts` wrapping `@tauri-apps/api/event` `listen` with typed event maps
- [ ] T030 [P] Create `src/lib/xterm/createTerminal.ts` factory returning a configured xterm.js `Terminal` + `FitAddon` + `WebLinksAddon`, disposed on unmount; single visual-smoke route at `src/routes/+page.svelte` that mounts one terminal and writes "workbench ready"

**Checkpoint**: Running `pnpm tauri dev` opens the workbench window, creates `workbench.db` with migrations applied, the daemon socket is listening, and the frontend mounts an xterm. Nothing functional yet — ready for US1.

---

## Phase 3: User Story 1 — See every active agent session at a glance (P1) 🎯 MVP slice 1

**Goal**: Projects can be registered, the CLI wrapper can register a session, that session appears in the workbench sidebar with live status, and removing/ending the session updates the list automatically.

**Independent Test**: Register two projects, run `agentui run -p <project> -- bash -c 'sleep 30'` in two different terminals against two different projects, open the workbench, confirm both sessions appear grouped under their projects with `working` status, wait for them to finish, confirm both transition to `ended` and are removed (or moved to an ended section).

### Contract tests for US1 (write FIRST, ensure RED)

- [ ] T031 [P] [US1] Contract test `project_register` happy path + `PATH_NOT_FOUND` + `PATH_NOT_A_DIRECTORY` + `ALREADY_REGISTERED` variants in `src-tauri/tests/contract/project_register.rs`
- [ ] T032 [P] [US1] Contract test `project_list` with `include_archived` flag in `src-tauri/tests/contract/project_list.rs`
- [ ] T033 [P] [US1] Contract test `project_update` (display_name, settings) + `NOT_FOUND` in `src-tauri/tests/contract/project_update.rs`
- [ ] T034 [P] [US1] Contract test `project_archive` + `project_unarchive` including `NOT_ARCHIVED` and the invariant that scratchpad rows survive archival in `src-tauri/tests/contract/project_archive.rs`
- [ ] T035 [P] [US1] Contract test `session_list` filtered by project and by `include_ended` in `src-tauri/tests/contract/session_list.rs`
- [ ] T036 [P] [US1] Contract test daemon IPC `hello` → `welcome`, including `protocol_version` mismatch → `PROTOCOL_ERROR` in `src-tauri/tests/contract/daemon/hello.rs`
- [ ] T037 [P] [US1] Contract test daemon IPC `register_session` happy path + creates project if unknown + `PATH_NOT_FOUND` in `src-tauri/tests/contract/daemon/register_session.rs`
- [ ] T038 [P] [US1] Contract test daemon IPC `update_status` status enum validation + `NOT_FOUND` for unknown session in `src-tauri/tests/contract/daemon/update_status.rs`
- [ ] T039 [P] [US1] Contract test daemon IPC `heartbeat` + `end_session` (exit code propagation) in `src-tauri/tests/contract/daemon/lifecycle.rs`
- [ ] T040 [P] [US1] Integration test: end-to-end `CLI wrapper → daemon IPC → sessions table` using a real temp socket, verifying `session_list` returns the registered row in `src-tauri/tests/integration/cli_to_list.rs`
- [ ] T041 [P] [US1] Integration test: session whose child process exits marks the row as `ended` within the status monitor's reaction window in `src-tauri/tests/integration/session_crash.rs`
- [ ] T041b [P] [US1] Contract test `session_spawn` happy path + `PROJECT_NOT_FOUND` + `PROJECT_ARCHIVED` + `WORKING_DIRECTORY_INVALID` + `SPAWN_FAILED { os_error }` in `src-tauri/tests/contract/session_spawn.rs`. MUST be RED before T049 implements `session_spawn`. Closes the TDD gate asserted in plan.md Post-Design Constitution Re-check.

### Backend: project service & commands

- [ ] T042 [P] [US1] Create `src-tauri/src/project/mod.rs` with `ProjectService` exposing `register(path, display_name) -> Project`, canonical-path resolution via `std::fs::canonicalize`, duplicate detection (`ALREADY_REGISTERED` returns existing id), and project row insertion with `created_at = now()`
- [ ] T043 [P] [US1] Create `src-tauri/src/project/watcher.rs` with a `notify` watcher per project emitting `project:path_missing` and `project:path_restored` events via `WorkbenchState::event_bus`
- [ ] T044 [US1] Create `src-tauri/src/commands/projects.rs` implementing `project_list`, `project_register`, `project_update`, `project_archive`, `project_unarchive` as `#[tauri::command]` functions delegating to `ProjectService`; register in `generate_handler![]` in `lib.rs`

### Backend: session actor, service, commands

- [ ] T045 [P] [US1] Create `src-tauri/src/session/live.rs` with `LiveSession` struct owning the `Pty`, reader/writer channels, and a status-monitor task handle; `LiveSessionHandle` (Clone) exposing `write(bytes)`, `resize(cols, rows)`, `end(signal)`, `status_rx()`, `output_rx()`
- [ ] T046 [P] [US1] Create `src-tauri/src/session/status.rs` with a minimal cooperative-first status monitor: listens for `update_status` signals from the daemon IPC handler, falls back to basic output-activity idle detection (`Instant::now() - last_output > 5s` ⇒ `idle`); heuristic prompt-pattern detection deferred to US2
- [ ] T047 [P] [US1] Create `src-tauri/src/session/supervisor.rs` that spawns per-session tokio tasks: a reader task forwarding PTY output to `event_bus` as `session:event { chunk }`, a writer task, and a monitor task calling `status::run(…)`
- [ ] T048 [US1] Create `src-tauri/src/session/mod.rs` with `SessionService` exposing `create_from_ipc(project_id, label, working_directory, command, pid, metadata) -> Session`, `list(project_id, include_ended) -> Vec<SessionSummary>`, `mark_ended(id, exit_code)`, `set_status(id, status, source)`
- [ ] T049 [US1] Create `src-tauri/src/commands/sessions.rs` with `session_list` and `session_spawn` (for future GUI-initiated spawns; delegates to `SessionService::spawn_local` which uses `Pty::spawn` directly); register handlers in `lib.rs`

### Backend: daemon IPC handlers

- [ ] T050 [US1] Implement `src-tauri/src/daemon/handlers.rs::dispatch` for `Hello` → `Welcome { server_version, protocol_version: 1, session_id_format: "i64" }`, rejecting mismatched `protocol_version` with `PROTOCOL_ERROR`
- [ ] T051 [US1] Implement `dispatch` for `RegisterSession`: canonicalize `project_path`, call `ProjectService::ensure_exists` (creates project if unknown using basename as display name), create session row via `SessionService::create_from_ipc`, spawn a `LiveSession` actor (but the CLI wrapper owns the PTY — actor runs in "attached-mirror" mode, owning no PTY, forwarding update_status inbound), return `SessionRegistered { session_id, project_id }`
- [ ] T052 [US1] Implement `dispatch` for `UpdateStatus` (session existence check, enum validation, updates row + broadcasts on `event_bus`), `Heartbeat` (updates `last_heartbeat_at`), `EndSession` (calls `SessionService::mark_ended`, broadcasts `session:ended`)

### Event emission

- [ ] T053 [US1] Create `src-tauri/src/commands/events.rs` with a tokio task launched in `run()` that bridges `state.event_bus` → `AppHandle::emit("session:spawned", …)`, `emit("session:ended", …)`, `emit("session:event", …)`, `emit("project:path_missing", …)`, `emit("project:path_restored", …)` — one match over an enveloped `SessionEventEnvelope` type

### CLI wrapper (agentui run)

- [ ] T054 [P] [US1] Create `cli/src/args.rs` with clap derive: `agentui run [-p|--project <name|path>] [-l|--label <string>] [--] <command>...`
- [ ] T055 [P] [US1] Create `cli/src/ipc.rs` with `IpcClient::connect($AGENTUI_SOCKET or XDG default)`, `send<T: Serialize>(frame)`, `recv<R: DeserializeOwned>()`, `hello()`, `register_session(payload)`, `heartbeat(id)`, `update_status(id, status)`, `end_session(id, exit_code)` — uses the same `protocol.rs` type definitions via a path dep from `src-tauri` (or a copy, noted as a TODO to extract to a shared crate post-v1)
- [ ] T056 [P] [US1] Create `cli/src/pty.rs` wrapping `portable-pty` on the CLI side: `run_child(command: &[String], cwd: &Path) -> PtyChild`, with `spawn_proxy(child, stdin, stdout)` that forwards user→PTY and PTY→user so the terminal behaves identically to running the child directly
- [ ] T057 [US1] Create `cli/src/main.rs`: parse args → resolve project (argument takes precedence; otherwise `$PWD`) → connect to socket → `hello` → resolve working dir → spawn child via `pty::run_child` → `register_session` → spawn a proxy task (PTY ↔ tty) + a status-monitor task (same library as backend — for US1, just calls `update_status(Working)` on start and lets backend infer idle) → await child exit → `end_session` → exit with child's code
- [ ] T058 [P] [US1] Integration test in `cli/tests/happy_path.rs`: start mock workbench socket from `tests/common`, run `agentui run -p test -- /bin/echo hi`, assert `hello` + `register_session` + `end_session` frames received in order, child exited 0

### Frontend: sidebar & session list

- [ ] T059 [P] [US1] Create `src/lib/api/projects.ts` with typed wrappers: `projectList({ includeArchived? })`, `projectRegister({ path, displayName? })`, `projectUpdate`, `projectArchive`, `projectUnarchive`
- [ ] T060 [P] [US1] Create `src/lib/api/sessions.ts` with typed wrappers: `sessionList({ projectId?, includeEnded? })`, `sessionSpawn(…)`, event subscribers `onSessionSpawned`, `onSessionEnded`, `onSessionEvent`
- [ ] T061 [P] [US1] Create `src/lib/stores/projects.svelte.ts` using Svelte 5 runes: `$state` for projects, derived `activeProjects`, methods `hydrate()`, `register(path)`, `archive(id)`; subscribes to relevant events on mount
- [ ] T062 [P] [US1] Create `src/lib/stores/sessions.svelte.ts` using runes: `$state` for sessions keyed by id, `$derived` for grouped-by-project, methods `hydrate()`, `add(session)`, `update(id, patch)`, `remove(id)`; reacts to `session:spawned`, `session:ended`, `session:event`
- [ ] T063 [P] [US1] Create `src/lib/components/Sidebar.svelte` listing projects, "Add Project" button that prompts for a path and calls `projectRegister`, archived-toggle checkbox
- [ ] T064 [P] [US1] Create `src/lib/components/SessionList.svelte` grouped by project with filter-by-text input (`FR-006`), and `src/lib/components/SessionRow.svelte` showing project name, label, status badge (working/idle/needs_input/ended/error) — no activity summary yet (US4)
- [ ] T065 [US1] Wire `src/routes/+page.svelte` to render `Sidebar` + `SessionList`, call `projects.hydrate()` + `sessions.hydrate()` on mount, subscribe to session events

**Checkpoint**: Run `pnpm tauri dev`. In another terminal, run `agentui run -p <registered-project> -- bash`. The workbench sidebar lists the project, a session appears under it with `working`, `Ctrl+D` in the wrapped bash exits → session transitions to `ended`. US1 is independently demoable.

---

## Phase 4: User Story 2 — Alerts when a session needs attention (P1)

**Goal**: Sessions that block on input are detected, marked `needs_input`, raise an OS notification (respecting quiet hours), display an in-app alert bar, and clear automatically when they resume.

**Independent Test**: Run `agentui run -p test -- bash -c 'read -p "y/n> " x; echo $x'` against the workbench. Within a few seconds, the session is flagged `needs_input`, an OS notification fires, the alert bar shows the session. Typing `y` in the real terminal clears the alert.

### Contract + integration tests (write FIRST)

- [ ] T066 [P] [US2] Contract test `session_acknowledge_alert` + `alert:cleared` event in `src-tauri/tests/contract/alerts.rs`
- [ ] T067 [P] [US2] Contract test `notification_preference_get` + `notification_preference_set` (global default + per-project override) in `src-tauri/tests/contract/notification_preferences.rs`
- [ ] T068 [P] [US2] Contract test daemon IPC `emit_alert` (treated as `update_status(NeedsInput)` in v1 per contracts doc) in `src-tauri/tests/contract/daemon/emit_alert.rs`
- [ ] T069 [P] [US2] Integration test: PTY session that writes a known prompt pattern is flagged `needs_input` within 2 s by the heuristic monitor in `src-tauri/tests/integration/needs_input_heuristic.rs`
- [ ] T070 [P] [US2] Integration test: `update_status(Working)` after `NeedsInput` automatically clears the alert and emits `alert:cleared { by: "session_resumed" }` in `src-tauri/tests/integration/alert_autoclear.rs`
- [ ] T071 [P] [US2] Integration test: quiet-hours policy suppresses OS notification but still emits in-app `alert:raised` event in `src-tauri/tests/integration/quiet_hours.rs`

### Backend: alert service + heuristic status

- [ ] T072 [P] [US2] Create `src-tauri/src/notifications/alerts.rs` with `AlertService::raise(session_id, kind, reason) -> Alert`, `clear(alert_id, by)`, `list_open(session_id)`, deduping identical consecutive raises within a 2 s window
- [ ] T073 [US2] Extend `src-tauri/src/session/status.rs` with heuristic prompt detection: maintain a rolling last-N-bytes buffer per session, match against a small pattern set (`\[y/N\]`, `password:`, `continue?`, `> ` at EOL after inactivity ≥ 1 s), flip status to `NeedsInput` with `StatusSource::Heuristic` and call `AlertService::raise`
- [ ] T074 [US2] Add status transition hooks in `session/mod.rs::set_status` that call `AlertService::clear(…, "session_resumed")` when a session leaves `NeedsInput` for `Working` or `Idle`, and `"session_ended"` when it enters `Ended`/`Error`

### Backend: notification preferences + Tauri plugin glue

- [ ] T075 [P] [US2] Create `src-tauri/src/notifications/preferences.rs` with `PreferenceService::get(project_id?)` resolving per-project → global default, `set(project_id?, channels, quiet_hours)`, channel enum (`OsNotification`, `InAppOnly`, `Silent`), `QuietHours { start: NaiveTime, end: NaiveTime, tz: "local" }`
- [ ] T076 [P] [US2] Create `src-tauri/src/notifications/mod.rs` with `dispatch_alert(alert, prefs)` that checks quiet hours, then calls `tauri_plugin_notification::Notification::new` for `OsNotification` channel
- [ ] T077 [US2] Wire alert bus → `dispatch_alert` in the event bridge task (from T053) so raised alerts fire OS notifications

### Backend: commands + events

- [ ] T078 [US2] Create `src-tauri/src/commands/notifications.rs` with `notification_preference_get`, `notification_preference_set`, and `session_acknowledge_alert` (calls `AlertService::clear(alert_id, "user")`); register in `lib.rs`
- [ ] T079 [US2] Emit `alert:raised`, `alert:cleared` events via the bridge task

### Frontend

- [ ] T080 [P] [US2] Create `src/lib/api/notifications.ts` wrappers + event subscribers `onAlertRaised`, `onAlertCleared`
- [ ] T081 [P] [US2] Extend `sessions.svelte.ts` store with `alerts: Map<SessionId, Alert>` keyed by session, `$derived` count of open alerts; reacts to alert events
- [ ] T082 [P] [US2] Create `src/lib/components/AlertBar.svelte` pinned above the session list, showing all open alerts with project + session label + reason + "acknowledge" button, click → `sessionAcknowledgeAlert`
- [ ] T083 [P] [US2] Create `src/lib/components/SettingsDialog.svelte` with notification preferences form (channel select, quiet hours start/end); wire to `notification_preference_get`/`set`
- [ ] T084 [US2] Update `SessionRow.svelte` to show a `needs_input` badge with pulsing style when the session has an open alert

**Checkpoint**: Running a session that prompts for input triggers an alert bar entry and an OS notification; responding clears it.

---

## Phase 5: User Story 3 — Jump to any session with a paired working terminal (P1)

**Goal**: Clicking a session in the list brings up a split view with the agent's PTY output (left pane) and a companion shell (right pane) that starts in the session's working directory. The companion is created on first activation, reused on subsequent activations, and transparently respawned if it's been killed.

**Independent Test**: With at least four sessions across three projects running, click each one. Each should open a split view showing the correct agent output and a companion shell already `cd`'d into that session's working directory. Close the companion pane's shell (`exit`), click the session again — a new companion spawns in the same directory. Use the filter to narrow by project name.

### Contract + integration tests (write FIRST)

- [ ] T085 [P] [US3] Contract test `session_activate` returning `{ session, companion }`, first-activation spawns companion, second-activation reuses it, killed companion is respawned, including `SESSION_ENDED` and `COMPANION_SPAWN_FAILED` in `src-tauri/tests/contract/session_activate.rs`
- [ ] T086 [P] [US3] Contract test `session_send_input`, `session_resize`, `session_end` in `src-tauri/tests/contract/session_io.rs`
- [ ] T087 [P] [US3] Contract test `companion_send_input`, `companion_resize`, `companion_respawn` in `src-tauri/tests/contract/companion.rs`
- [ ] T088 [P] [US3] Integration test: worktree path — session's `working_directory` differs from project root; companion spawns in the worktree, not the root, per FR-015 + Edge Cases. Same test file asserts FR-017 negative invariant: after the user writes `cd /tmp\n` into the companion, a subsequent `companion_resize` and a follow-up `session_activate` of the same session MUST NOT reset the companion's cwd back to the session root (verified by sending `pwd\n` and asserting `/tmp` in the output stream). In `src-tauri/tests/integration/companion_worktree.rs`.
- [ ] T089 [P] [US3] Integration test: activation creates exactly one `companion_terminals` row per session (idempotency) in `src-tauri/tests/integration/companion_idempotency.rs`

### Backend: companion terminal service

- [ ] T090 [P] [US3] Create `src-tauri/src/companion/mod.rs` with `CompanionService::ensure(session_id) -> CompanionTerminal` that: looks up existing row, if any checks whether its PTY is still alive (via `sysinfo::ProcessRefreshKind`), if alive returns it, if dead marks old row `ended_at = now()` and spawns a new shell (`$SHELL` or `/bin/bash`) in the session's `working_directory`, registers a new `CompanionTerminal` row, creates a `LiveCompanion` actor paralleling `LiveSession`, returns the new row
- [ ] T091 [US3] Wire companion reader output → `event_bus` as `companion:output { session_id, bytes }` events via supervisor task

### Backend: commands

- [ ] T092 [US3] Implement `session_activate` in `src-tauri/src/commands/sessions.rs`: validates session is not `ended`, calls `CompanionService::ensure`, returns `{ session, companion }`, emits `companion:spawned` on fresh creation
- [ ] T093 [US3] Implement `session_send_input`, `session_resize`, `session_end` in `commands/sessions.rs` — each dispatches to the `LiveSessionHandle` from `state.live_sessions`; v1 note: for attached-mirror sessions (daemon-IPC-owned), `session_send_input` returns `WRITE_FAILED` with details `"input must be sent to the wrapping terminal"` per the research.md §6 tradeoff
- [ ] T094 [P] [US3] Create `src-tauri/src/commands/companions.rs` implementing `companion_send_input`, `companion_resize`, `companion_respawn`; register all in `lib.rs`

### Frontend: split view & panes

- [ ] T095 [P] [US3] Create `src/lib/api/companions.ts` with typed wrappers + event subscribers `onCompanionSpawned`, `onCompanionOutput`
- [ ] T096 [P] [US3] Create `src/lib/components/AgentPane.svelte` mounting an xterm.js instance, subscribing to `session:event` chunks for the active session, dispatching keystrokes to `sessionSendInput` (gracefully degrading if the backend returns `WRITE_FAILED` for attached-mirror sessions — displays a "input this session from your launching terminal" banner)
- [ ] T097 [P] [US3] Create `src/lib/components/CompanionPane.svelte` mounting an xterm.js instance, subscribing to `companion:output` for the active session, dispatching keystrokes to `companionSendInput`; handles `companion_resize` on container resize via `FitAddon`
- [ ] T098 [US3] Create `src/lib/components/SplitView.svelte` rendering `AgentPane` + `CompanionPane` horizontally with a draggable divider, calling `sessionActivate(id)` on mount to fetch the session + companion, emitting lifecycle cleanup on unmount
- [ ] T099 [US3] Wire `SessionRow.svelte` click handler to set `activeSessionId` in the sessions store; `+page.svelte` renders `SplitView` when `activeSessionId !== null`
- [ ] T100 [P] [US3] Extend `SessionList.svelte` with a project-name filter input (debounced 150 ms) — completes FR-006 started in US1

**Checkpoint**: Click any session → split view shows agent output + a working shell in the right directory. Killing the companion shell and re-clicking respawns it. Filter by project name narrows the list.

---

## Phase 6: User Story 5 — Per-project scratchpad + cross-project overview (P1)

**Goal**: Each project has a persistent scratchpad holding free-form notes and checkable reminders. The scratchpad survives workbench restart, session end, and project archival (archived, not deleted). A cross-project overview rolls up every open reminder across every registered project, grouped by project, with age visible.

**Independent Test**: Register two projects, add two notes and two reminders to each, close the workbench, reopen it, open each project — notes + reminders are there. Open the cross-project overview — all four reminders are grouped under their projects with age. Mark one done — it disappears from the overview but is still retrievable in that project's scratchpad history. Archive the project, unarchive it — the scratchpad is intact.

### Contract + integration tests (write FIRST)

- [ ] T101 [P] [US5] Contract tests for `note_list`, `note_create` (including `CONTENT_EMPTY`, `PROJECT_NOT_FOUND`), `note_update`, `note_delete` in `src-tauri/tests/contract/notes.rs`
- [ ] T102 [P] [US5] Contract tests for `reminder_list` (state filter), `reminder_create`, `reminder_set_state`, `reminder_delete` in `src-tauri/tests/contract/reminders.rs`
- [ ] T103 [P] [US5] Contract test `cross_project_overview` returning groups ordered by project with per-group reminders ordered `created_at DESC` in `src-tauri/tests/contract/overview.rs`
- [ ] T104 [P] [US5] Integration test: archive project → all its notes + reminders still queryable; unarchive → scratchpad intact, session rows still ended in `src-tauri/tests/integration/scratchpad_archive.rs`
- [ ] T105 [P] [US5] Integration test: ending a session leaves its project's scratchpad untouched, AND the FR-027 negative invariant holds — running a session that produces several KB of PTY output (including text that looks like a note or a reminder) MUST NOT create, mutate, or delete any row in `notes` or `reminders` for that session's project. Verified by snapshotting both tables before and after the session's lifecycle. In `src-tauri/tests/integration/scratchpad_session_lifecycle.rs`.
- [ ] T106 [P] [US5] Integration test: done reminder excluded from `cross_project_overview` but retrievable via `reminder_list({ state: "done" })` in `src-tauri/tests/integration/reminder_done.rs`

### Backend: scratchpad service + commands

- [ ] T107 [P] [US5] Create `src-tauri/src/scratchpad/notes.rs` with `NoteService::list(project_id, limit, cursor)`, `create(project_id, content)` (rejects empty/whitespace-only with `CONTENT_EMPTY`), `update(id, content)`, `delete(id)`
- [ ] T108 [P] [US5] Create `src-tauri/src/scratchpad/reminders.rs` with `ReminderService::list(project_id?, state?, limit, cursor)`, `create(project_id, content)`, `set_state(id, state)`, `delete(id)`
- [ ] T109 [P] [US5] Create `src-tauri/src/scratchpad/overview.rs` with `overview() -> Vec<OverviewGroup>` joining `projects` × open `reminders`, grouped by project, ordered by project display_name then reminder `created_at DESC`; excludes archived projects
- [ ] T110 [US5] Create `src-tauri/src/commands/scratchpad.rs` implementing `note_list`, `note_create`, `note_update`, `note_delete`, `reminder_list`, `reminder_create`, `reminder_set_state`, `reminder_delete`, `cross_project_overview`; register in `lib.rs`

### Frontend: scratchpad UI

- [ ] T111 [P] [US5] Create `src/lib/api/scratchpad.ts` with typed wrappers for all 9 scratchpad commands
- [ ] T112 [P] [US5] Create `src/lib/util/age.ts` returning human-readable age ("3 minutes ago", "2 days ago", "3 weeks ago") from an ISO-8601 timestamp
- [ ] T113 [P] [US5] Create `src/lib/util/markdown.ts` wrapping `marked` + `DOMPurify` for light inline-markdown rendering (bold, italic, code, links — block elements disabled)
- [ ] T114 [P] [US5] Create `src/lib/stores/scratchpad.svelte.ts` (per-project notes + reminders, lazy-loaded on project open) and `src/lib/stores/overview.svelte.ts` (cross-project, re-queried on reminder state changes)
- [ ] T115 [P] [US5] Create `src/lib/components/Scratchpad.svelte` with two tabs (Notes / Reminders), textarea + "Add note" button, reminder list with checkboxes (click → `reminderSetState`), age display next to each reminder, inline markdown render on notes
- [ ] T116 [P] [US5] Create `src/lib/components/CrossProjectOverview.svelte` rendering groups with project header and reminder list, each reminder showing its age and project
- [ ] T117 [US5] Integrate `Scratchpad` as a toggleable right-side panel in `SplitView.svelte` (keyboard shortcut to toggle), and add an "Overview" top-nav button that opens `CrossProjectOverview` as a full-width view

**Checkpoint**: Notes + reminders persist across workbench restarts; the overview rolls up across projects; archived projects still return their scratchpad on unarchive.

---

## Phase 7: User Story 6 — Persistent workspace state + named layouts (P1)

**Goal**: Closing and reopening the workbench restores the exact workspace (registered projects, last active session, panel sizes, which companion was showing) automatically. Still-running sessions reattach; dead ones are marked `not running`. Users can additionally save named layouts and switch between them explicitly.

**Independent Test**: Start the workbench, register three projects, run an `agentui run -- sleep 300` session in each, activate one with the scratchpad panel open, close the workbench. Reopen — same projects, same last-active session, same panel state, all three sessions still attached. Save that state as layout "triple-repo"; register a fourth project, activate its new session, save as layout "fourth-project-focus"; restore "triple-repo" — back to the previous state without manual re-entry.

### Contract + integration tests (write FIRST)

- [ ] T118 [P] [US6] Contract test `workspace_get` returns a hydrateable state (roundtrippable with `workspace_save`) in `src-tauri/tests/contract/workspace.rs`
- [ ] T119 [P] [US6] Contract tests `layout_list`, `layout_save` (+ `NAME_TAKEN`), `layout_restore` (returns `missing_sessions` for dead refs), `layout_delete` in `src-tauri/tests/contract/layouts.rs`
- [ ] T120 [P] [US6] Integration test: close-and-reopen simulation — save workspace state row, restart `WorkbenchState`, run crash recovery, verify still-alive sessions (mocked via pid of a long-running helper) reattach and dead pids are marked `ended` in `src-tauri/tests/integration/workspace_restore.rs`
- [ ] T121 [P] [US6] Integration test: `layout_restore` with a layout referencing an `ended` session reports it in `missing_sessions` and the UI-level hydration marks it `not running` in `src-tauri/tests/integration/layout_missing.rs`

### Backend: workspace + layout services

- [ ] T122 [P] [US6] Create `src-tauri/src/workspace/mod.rs` with `WorkspaceService::get()` (reads the single `workspace_state` row; if absent returns `WorkspaceState::default()`), `save(state)` with debounced write (100 ms) coalescing rapid consecutive saves, `flush()` called on graceful shutdown
- [ ] T123 [P] [US6] Create `src-tauri/src/workspace/layouts.rs` with `LayoutService::list()`, `save(name, state)` (`NAME_TAKEN` on duplicate), `restore(id) -> (WorkspaceState, Vec<SessionId>)` that returns missing session ids after checking which referenced sessions are still alive in the live-sessions map, `delete(id)`
- [ ] T124 [US6] Extend `src-tauri/src/session/recovery.rs` with `reattach_live_sessions(state)` that scans `sessions` rows whose `ended_at IS NULL`, checks each pid via `sysinfo`, for alive pids creates attached-mirror `LiveSession` handles with `status_source = Heuristic` (no cooperative IPC until the wrapper reconnects), for dead pids calls `mark_ended` — wired into `run()` after DB open

### Backend: commands + events

- [ ] T125 [US6] Create `src-tauri/src/commands/workspace.rs` with `workspace_get`, `workspace_save`, `layout_list`, `layout_save`, `layout_restore`, `layout_delete`; register in `lib.rs`
- [ ] T126 [US6] Emit `workspace:restored` event from the event bridge on startup, containing the hydrated state

### Frontend: workspace hydration + layouts

- [ ] T127 [P] [US6] Create `src/lib/api/workspace.ts` with typed wrappers + `onWorkspaceRestored` event subscriber
- [ ] T128 [P] [US6] Create `src/lib/stores/workspace.svelte.ts` with current workspace state ($state), debounced (250 ms) `save()` that pushes to backend, `hydrate()` called before any other store's hydrate
- [ ] T129 [P] [US6] Create `src/lib/components/LayoutSwitcher.svelte` — top-nav dropdown listing layouts + "save current as…" + "delete" actions
- [ ] T130 [US6] Wire stores and components to call `workspace.save()` on UI changes (active session, panel sizes, scratchpad toggle state); on mount, call `workspace.hydrate()` first, then `projects.hydrate()` + `sessions.hydrate()`; mark sessions returned as missing from restore in the UI with a muted "not running" badge

**Checkpoint**: Closing and reopening restores the full workbench automatically. Named layouts switch explicitly.

---

## Phase 8: User Story 4 — Session activity summary (P2)

**Goal**: Each session row in the list shows a short human-readable activity summary (last meaningful output line, idle time, or agent-provided task title) so the user can tell sessions apart without opening each one. Ephemeral and distinct from the persistent scratchpad.

**Independent Test**: Run three sessions doing visibly different things (one running `while true; do date; sleep 1; done`, one `sleep 60`, one `read -p "prompt> " x`). The session list shows a distinct, short summary per row (a recent timestamp line, an idle timer, the prompt line) — each updating live.

### Contract + integration tests (write FIRST)

- [ ] T131 [P] [US4] Contract test: `SessionSummary` returned by `session_list` includes `activity_summary: string | null` and, when the session has an open alert, `alert: Alert | null` in `src-tauri/tests/contract/session_summary.rs`
- [ ] T132 [P] [US4] Integration test: after 300 output chunks, the ring buffer contains only the last N lines and the summary is the most-recent non-empty line in `src-tauri/tests/integration/activity_ring_buffer.rs`
- [ ] T133 [P] [US4] Integration test: `update_status { summary: "Refactoring lexer" }` overrides heuristic-derived summary until the next `update_status` or until output activity resumes past a timeout in `src-tauri/tests/integration/activity_override.rs`

### Backend

- [ ] T134 [P] [US4] Create `src-tauri/src/session/activity.rs` with `ActivitySummary` holding a bounded (8 KiB) ring buffer of recent output + last-line extraction; `record_chunk(bytes)`, `override_with(summary)` (expires after 10 s of continued output or 60 s absolute), `current() -> Option<String>`
- [ ] T135 [US4] Store one `ActivitySummary` per `LiveSession`; reader task in `session/supervisor.rs` calls `record_chunk` on each PTY chunk
- [ ] T136 [US4] Extend `SessionService::list` to populate `SessionSummary { session, activity_summary: live_session.activity.current(), alert: alert_service.current(session.id) }` and update `session_list` contract accordingly

### Frontend

- [ ] T137 [US4] Extend `SessionRow.svelte` to render activity summary below the label (truncated to ~60 chars with an ellipsis + tooltip), an idle-time counter for `idle` sessions, and an optional task-title pill when the session metadata includes one

**Checkpoint**: Session list rows are visibly distinct per session's activity. P2 delivered after all P1 stories.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: End-to-end validation, coverage gate, performance sanity, quickstart exercise, documentation touch-ups.

- [ ] T138 [P] Create `tests/e2e/first-run.spec.ts` (Playwright via `tauri-driver`): launches the app, asserts empty-state "No projects registered", clicks "Add Project", selects a temp dir, confirms the project appears
- [ ] T139 [P] Create `tests/e2e/session-lifecycle.spec.ts`: registers a project, simulates a daemon-IPC `register_session` via a helper, asserts the session appears, activates it, asserts split view mounts, ends the session, asserts it transitions to `ended`
- [ ] T140 [P] Create `tests/e2e/scratchpad.spec.ts`: adds notes and reminders, relaunches, verifies persistence, opens cross-project overview, marks a reminder done
- [ ] T141 [P] Create `tests/e2e/workspace-restore.spec.ts`: saves a layout, restarts the app, asserts workspace state restored automatically and layout dropdown shows the saved one
- [ ] T142 Create `tests/integration/session_to_scratchpad.rs` verifying that a session ending does NOT delete or modify any note or reminder in its project
- [ ] T143 Install and run `cargo tarpaulin --out Html --output-dir target/coverage --workspace --exclude-files "tests/*"` in CI locally; assert ≥ 80 % for `src-tauri/` and `cli/`. Document any justified exclusions inline in the offending modules
- [ ] T144 Performance sanity pass: spawn 10 long-running sessions across 5 mock projects (helper script), time `session_list` + filter + activation; assert < 100 ms list, < 200 ms activation (SC-004). Also seed 5 000 notes + 5 000 reminders into a single project and assert `note_list` (paginated) and `cross_project_overview` both return within the same 100 ms list budget (covers the long-scratchpad edge case from spec.md). Record all results in `specs/001-cross-repo-workbench/quickstart.md` under "Performance check"
- [ ] T145 [P] Accessibility pass: run `pnpm --filter frontend lint:a11y` (axe-core via Vitest component tests) on `Sidebar`, `SessionList`, `SessionRow`, `AlertBar`, `Scratchpad`, `CrossProjectOverview`, `SplitView`; fix any Serious/Critical issues
- [ ] T146 [P] Update `specs/001-cross-repo-workbench/quickstart.md` if any step required tweaks during implementation; add a "Dev troubleshooting" subsection for gotchas encountered
- [ ] T147 [P] Write project `README.md` at `/home/knitli/agentui/README.md` with: one-paragraph description, link to `specs/001-cross-repo-workbench/` for the full spec/plan, quickstart commands, known v1 limitations (copied from `research.md` §16)
- [ ] T148 Run the full quickstart exercise end-to-end (`quickstart.md` §3–§5) from a freshly-built binary; confirm every acceptance scenario from `spec.md` passes. Check off each on a fresh copy of `checklists/requirements.md` if desired
- [ ] T149 Final sweep: `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`, `pnpm --filter frontend lint`, `pnpm --filter frontend typecheck`. Additionally enforce the FR-022 local-only scope fence: grep the `src-tauri/` and `cli/` source trees for `TcpStream`, `TcpListener`, `reqwest`, `hyper::client`, `ssh`, `\\wsl$`, and any `//` or `wss://`/`https://` URL literals; any hit must either be inside a test, behind a documented v2-future feature flag, or removed. Zero errors, zero warnings, zero unexplained network hits before calling this done

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1 Setup** → no deps
- **Phase 2 Foundational** → depends on Phase 1; blocks all user stories
- **Phase 3 US1 (unified overview)** → depends on Phase 2; blocks US2, US3, US6 (all need the session & project entities and daemon-IPC handlers)
- **Phase 4 US2 (alerts)** → depends on US1 (sessions must exist to alert on)
- **Phase 5 US3 (activation + companion)** → depends on US1 (sessions); technically parallel with US2 once US1 is done
- **Phase 6 US5 (scratchpad)** → depends only on US1's project entity; can run fully in parallel with US2/US3
- **Phase 7 US6 (workspace state + layouts)** → depends on US1 (session entity) + US3 (companion entity needed in workspace state); realistically runs after US3
- **Phase 8 US4 (activity summary)** → depends on US1 (session rows); P2 enhancement, run last of the story phases
- **Phase 9 Polish** → depends on all desired user stories being complete

### Story-independence matrix

- **US1** standalone — the MVP slice that establishes projects + sessions.
- **US2** requires US1; independently testable after US1.
- **US3** requires US1; independently testable after US1 (US2 not required).
- **US5** requires only US1's project entity; independently testable after US1.
- **US6** realistically requires US1 + US3 (companion entity persists in workspace state); independently testable once those are in.
- **US4** requires only US1; independently testable, P2, run last.

### Within each user story

- Contract/integration tests (marked [P] within a story) MUST be written first and failing before implementation begins.
- Models and services before commands; commands before frontend.
- Story implementation-level [P] tasks = different files, no incomplete dependencies.

### Parallel opportunities

- **Phase 1 Setup**: T004–T009 all [P]; run together after T001–T003 create the workspace scaffold.
- **Phase 2 Foundational**: T013 (model files), T020 (PTY), T021 (protocol types), T024 (handler stub), T026 (tracing), T027–T030 (test infra + frontend foundation) are all [P] once T011, T012, T014, T015, T017, T018 are in.
- **Phase 3 US1**: All contract tests T031–T041b run in parallel. Services T042 (project) + T045–T047 (session actor pieces) + T054–T056 (CLI wrapper pieces) all parallelizable. Frontend files T059–T064 all parallelizable.
- **Phase 4 US2**: Tests T066–T071 parallel. Services T072, T075, T076 parallel. Frontend T080–T083 parallel.
- **Phase 5 US3**: Tests T085–T089 parallel. Backend T090 + T094 parallel. Frontend T095–T097, T100 parallel.
- **Phase 6 US5**: Tests T101–T106 parallel. Services T107–T109 parallel. Frontend T111–T116 parallel.
- **Phase 7 US6**: Tests T118–T121 parallel. Services T122, T123 parallel. Frontend T127–T129 parallel.
- **Phase 8 US4**: Tests T131–T133 parallel; T134 parallel with tests.
- **Phase 9 Polish**: T138–T141, T145, T146, T147 parallel.
- **Cross-story parallelism** after US1 lands: US2, US3, and US5 can proceed in parallel if multiple workers are available.

---

## Parallel Example: US1 contract tests

```bash
# Launch all US1 contract tests together (each is a separate test file):
Task: "Contract test project_register in src-tauri/tests/contract/project_register.rs"
Task: "Contract test project_list in src-tauri/tests/contract/project_list.rs"
Task: "Contract test project_update in src-tauri/tests/contract/project_update.rs"
Task: "Contract test project_archive in src-tauri/tests/contract/project_archive.rs"
Task: "Contract test session_list in src-tauri/tests/contract/session_list.rs"
Task: "Contract test daemon hello in src-tauri/tests/contract/daemon/hello.rs"
Task: "Contract test daemon register_session in src-tauri/tests/contract/daemon/register_session.rs"
Task: "Contract test daemon update_status in src-tauri/tests/contract/daemon/update_status.rs"
Task: "Contract test daemon lifecycle in src-tauri/tests/contract/daemon/lifecycle.rs"
Task: "Integration test cli_to_list in src-tauri/tests/integration/cli_to_list.rs"
Task: "Integration test session_crash in src-tauri/tests/integration/session_crash.rs"
```

## Parallel Example: US1 backend services

```bash
# Once tests are in (RED), launch service modules together:
Task: "ProjectService in src-tauri/src/project/mod.rs"
Task: "Project watcher in src-tauri/src/project/watcher.rs"
Task: "LiveSession actor in src-tauri/src/session/live.rs"
Task: "Status monitor in src-tauri/src/session/status.rs"
Task: "Session supervisor in src-tauri/src/session/supervisor.rs"
```

---

## Implementation Strategy

### MVP-of-the-MVP (US1 only)

1. Complete Phase 1 (Setup) and Phase 2 (Foundational).
2. Complete Phase 3 (US1): the workbench can register projects, the CLI wrapper registers sessions over the daemon socket, the sidebar lists sessions with live status.
3. **STOP and validate**: run `agentui run -- bash` and see the session appear. Confirm idle + ended transitions.
4. This is the smallest ship-able slice: "I can see all my agents across repos in one place."

### P1 full slice

1. MVP-of-the-MVP (above).
2. US2 (alerts): now "I see them AND I get pinged when one needs me."
3. US3 (activation + companion): now "I can also click one and have a shell ready in its repo."
4. US5 (scratchpad): now "I can also keep my own context per project and across projects."
5. US6 (workspace state + layouts): now "And all of it survives reboots."
6. Ship. Every P1 story in the spec is satisfied.

### P2 enhancement

1. US4 (activity summary): visible activity per session row.
2. Polish phase.

### Parallel team strategy

With multiple developers after US1 ships:

- Dev A: US2 (alerts) — mostly backend + a small frontend piece.
- Dev B: US3 (activation + companion) — bigger frontend surface.
- Dev C: US5 (scratchpad) — fully parallel backend + frontend; minimal cross-cut.
- Sync point: US6 starts once US3's companion entity is merged, because workspace state references companion rows.

---

## Notes

- [P] tasks touch different files and have no dependency on incomplete tasks in the same phase.
- Every `[USx]` task traces back to at least one FR in `spec.md`; most also map to a Tauri command or daemon IPC verb in `contracts/`.
- Contract tests for every Tauri command and daemon IPC verb MUST be written and failing before the implementation lands (per TDD in `plan.md` Constitution Check and `contracts/README.md`).
- Backend tests use `temp_db()` from `tests/common/mod.rs` (T027); no global state.
- Frontend unit tests (Vitest) are implicit per-component during the frontend implementation tasks; they pin the API wire format from the Svelte side and MUST be added alongside each store/component task. They are not broken out as individual T-entries to avoid doubling the task count — treat them as part of the Definition of Done for any frontend task.
- Commit after each task or logical group. Rebuild coverage after each phase; aim to never regress the 80 % gate on backend + CLI.
- Avoid introducing any feature beyond the 32 FRs in `spec.md` — the Constitution Check flagged scope discipline as a hard gate.

---

## Task count summary

| Phase | Tasks | Parallelizable |
|---|---|---|
| 1. Setup | 10 (T001–T010) | 6 |
| 2. Foundational | 20 (T011–T030) | 9 |
| 3. US1 — Unified overview (P1 MVP) | 36 (T031–T065, incl. T041b) | ~21 |
| 4. US2 — Alerts (P1) | 19 (T066–T084) | ~12 |
| 5. US3 — Activation + companion (P1) | 16 (T085–T100) | ~10 |
| 6. US5 — Scratchpad (P1) | 17 (T101–T117) | ~12 |
| 7. US6 — Workspace state + layouts (P1) | 13 (T118–T130) | ~8 |
| 8. US4 — Activity summary (P2) | 7 (T131–T137) | ~4 |
| 9. Polish | 12 (T138–T149) | ~7 |
| **Total** | **150** | **~89 [P]** |

Suggested MVP scope: **Phases 1 + 2 + 3 (T001–T065)** — the smallest slice that delivers the unified cross-repo session view and validates the entire backend/CLI/frontend pipeline end-to-end.
