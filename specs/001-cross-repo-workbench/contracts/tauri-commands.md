# Tauri Command Surface

All commands are registered via `tauri::generate_handler!` and invoked from the Svelte frontend via `@tauri-apps/api/core`'s `invoke`. Each command is async on the backend.

Conventions:

- Ids are serialized as numbers (SQLite rowids).
- Timestamps are serialized as ISO-8601 UTC strings.
- Errors return a structured `WorkbenchError { code: string, message: string, details?: object }` and map to JS rejections.
- Streaming updates (session output, status changes, alerts) are delivered via **Tauri events** (`app.emit`), not request/response.

---

## 1. Project management

### `project_list`

- **Request**: `{ include_archived?: boolean }` (default `false`)
- **Response**: `{ projects: Project[] }`
- **Purpose**: List registered projects. Powers the sidebar and the Cross-Project Overview.
- **Spec refs**: FR-001, FR-002, FR-018.

### `project_register`

- **Request**: `{ path: string, display_name?: string }`
- **Response**: `{ project: Project }`
- **Errors**: `PATH_NOT_FOUND`, `PATH_NOT_A_DIRECTORY`, `ALREADY_REGISTERED` (returns existing project id)
- **Behavior**: Canonicalizes `path`, creates a project row, initializes an empty scratchpad state, starts a filesystem watcher.
- **Spec refs**: FR-018, FR-023, FR-014.

### `project_update`

- **Request**: `{ id: ProjectId, display_name?: string, settings?: ProjectSettings }`
- **Response**: `{ project: Project }`
- **Errors**: `NOT_FOUND`

### `project_archive`

- **Request**: `{ id: ProjectId }`
- **Response**: `{}`
- **Behavior**: Soft-deletes the project, stops its filesystem watcher, ends and reaps all its live sessions (marks them `ended`), preserves all notes/reminders/scratchpad content.
- **Spec refs**: FR-032.

### `project_unarchive`

- **Request**: `{ id: ProjectId }`
- **Response**: `{ project: Project }`
- **Errors**: `NOT_ARCHIVED`, `PATH_NOT_FOUND` (original path gone)

---

## 2. Sessions

### `session_list`

- **Request**: `{ project_id?: ProjectId, include_ended?: boolean }` (default `include_ended = false`)
- **Response**: `{ sessions: SessionSummary[] }`
- **Purpose**: Overview query. `SessionSummary` is `Session` plus a derived `activity_summary: string | null` and current `alert: Alert | null`.
- **Spec refs**: FR-001, FR-003, FR-011.

### `session_spawn`

- **Request**: `{ project_id: ProjectId, label?: string, command: string[], working_directory?: string, env?: Record<string, string> }`
- **Response**: `{ session: Session }`
- **Behavior**: Spawns a child process under a PTY in the requested working directory (defaulting to project root), creates the session row, and returns immediately. Session output begins flowing via `session:event` events.
- **Errors**: `PROJECT_NOT_FOUND`, `PROJECT_ARCHIVED`, `WORKING_DIRECTORY_INVALID`, `SPAWN_FAILED { os_error }`
- **Spec refs**: FR-008 (via CLI wrapper path, see daemon IPC).

### `session_activate`

- **Request**: `{ session_id: SessionId }`
- **Response**: `{ session: Session, companion: CompanionTerminal }`
- **Behavior**: Brings the session's split view to the foreground in the workbench UI. If no companion terminal exists yet, spawns one in the session's `working_directory`. If the previous companion terminal is gone, transparently recreates it.
- **Errors**: `NOT_FOUND`, `SESSION_ENDED`, `COMPANION_SPAWN_FAILED`
- **Spec refs**: FR-007, FR-015, FR-016, US3.

### `session_send_input`

- **Request**: `{ session_id: SessionId, bytes: string }` (base64 or plain UTF-8)
- **Response**: `{}`
- **Behavior**: Writes bytes to the session's PTY stdin. Used by the embedded xterm.js on user keystrokes.
- **Errors**: `NOT_FOUND`, `SESSION_ENDED`, `WRITE_FAILED`

### `session_resize`

- **Request**: `{ session_id: SessionId, cols: number, rows: number }`
- **Response**: `{}`
- **Behavior**: Resizes the PTY (`TIOCSWINSZ`).

### `session_end`

- **Request**: `{ session_id: SessionId, signal?: "TERM" | "KILL" }` (default `TERM`)
- **Response**: `{ session: Session }`
- **Behavior**: Sends a signal to the child. Does not block on exit.

### `session_acknowledge_alert`

- **Request**: `{ session_id: SessionId, alert_id: AlertId }`
- **Response**: `{}`
- **Behavior**: Clears a `needs_input` alert manually (for cases where the user responds out of band and wants to silence the badge).
- **Spec refs**: FR-012.

---

## 3. Companion terminals

### `companion_send_input`

- **Request**: `{ session_id: SessionId, bytes: string }`
- **Response**: `{}`

### `companion_resize`

- **Request**: `{ session_id: SessionId, cols: number, rows: number }`
- **Response**: `{}`

### `companion_respawn`

- **Request**: `{ session_id: SessionId }`
- **Response**: `{ companion: CompanionTerminal }`
- **Behavior**: Forcibly recreates the companion terminal. Normally not needed — `session_activate` respawns transparently — but useful for the UI's explicit "restart terminal" button.

---

## 4. Scratchpad (Notes + Reminders)

### `note_list`

- **Request**: `{ project_id: ProjectId, limit?: number, cursor?: string }`
- **Response**: `{ notes: Note[], next_cursor?: string }`

### `note_create`

- **Request**: `{ project_id: ProjectId, content: string }`
- **Response**: `{ note: Note }`
- **Errors**: `PROJECT_NOT_FOUND`, `CONTENT_EMPTY`
- **Spec refs**: FR-024, FR-025.

### `note_update`

- **Request**: `{ id: NoteId, content: string }`
- **Response**: `{ note: Note }`

### `note_delete`

- **Request**: `{ id: NoteId }`
- **Response**: `{}`

### `reminder_list`

- **Request**: `{ project_id?: ProjectId, state?: "open" | "done", limit?: number, cursor?: string }`
- **Response**: `{ reminders: Reminder[], next_cursor?: string }`
- **Spec refs**: FR-024, FR-025, FR-028.

### `reminder_create`

- **Request**: `{ project_id: ProjectId, content: string }`
- **Response**: `{ reminder: Reminder }`

### `reminder_set_state`

- **Request**: `{ id: ReminderId, state: "open" | "done" }`
- **Response**: `{ reminder: Reminder }`
- **Spec refs**: FR-029.

### `reminder_delete`

- **Request**: `{ id: ReminderId }`
- **Response**: `{}`

### `cross_project_overview`

- **Request**: `{}`
- **Response**: `{ groups: { project: Project, open_reminders: Reminder[] }[] }`
- **Purpose**: Drives the Cross-Project Overview (FR-028). Results grouped by project, with per-project ordering `created_at DESC`.

---

## 5. Workspace state and layouts

### `workspace_get`

- **Request**: `{}`
- **Response**: `{ state: WorkspaceState }`
- **Behavior**: Returns the current in-memory workspace state. Called on frontend boot to hydrate the UI.
- **Spec refs**: FR-019, FR-020, US6.

### `workspace_save`

- **Request**: `{ state: WorkspaceState }`
- **Response**: `{}`
- **Behavior**: Debounced write to the `workspace_state` table. Called by the frontend on UI changes. Also flushed automatically by the backend on graceful shutdown.

### `layout_list`

- **Request**: `{}`
- **Response**: `{ layouts: Layout[] }`

### `layout_save`

- **Request**: `{ name: string, state: WorkspaceState }`
- **Response**: `{ layout: Layout }`
- **Errors**: `NAME_TAKEN` (prompt user to confirm overwrite from the UI)

### `layout_restore`

- **Request**: `{ id: LayoutId }`
- **Response**: `{ state: WorkspaceState, missing_sessions: SessionId[] }`
- **Behavior**: Loads the layout and reattaches to any sessions that are still alive. Reports missing ones so the UI can mark them "not running" per spec.
- **Spec refs**: US6.

### `layout_delete`

- **Request**: `{ id: LayoutId }`
- **Response**: `{}`

---

## 6. Notification preferences

### `notification_preference_get`

- **Request**: `{ project_id?: ProjectId }` (omit for global default)
- **Response**: `{ preference: NotificationPreference }`

### `notification_preference_set`

- **Request**: `{ project_id?: ProjectId, channels: NotificationChannel[], quiet_hours?: QuietHours }`
- **Response**: `{ preference: NotificationPreference }`
- **Spec refs**: FR-013.

---

## 7. Events (backend → frontend)

Emitted via `AppHandle::emit`. The frontend subscribes with `listen(eventName, handler)`.

| Event name | Payload | When |
|---|---|---|
| `session:event` | `{ session_id, event: SessionEvent }` | Any session state change, output chunk, or lifecycle transition. |
| `session:spawned` | `{ session: Session }` | New session appears (from GUI spawn or daemon registration). |
| `session:ended` | `{ session_id, code?: number }` | Child exit. |
| `alert:raised` | `{ alert: Alert }` | New `needs_input` alert. |
| `alert:cleared` | `{ alert_id, by: "user" \| "session_resumed" \| "session_ended" }` | Alert cleared. |
| `project:path_missing` | `{ project_id }` | Filesystem watcher detected the repo root disappeared. |
| `project:path_restored` | `{ project_id }` | Previously missing root is back. |
| `companion:spawned` | `{ session_id, companion }` | Companion terminal created or respawned. |
| `companion:output` | `{ session_id, bytes: string }` | Companion shell output chunk. |
| `workspace:restored` | `{ state }` | On launch, after initial hydration. |

Event payloads are versioned implicitly by a contract-test snapshot; changes require coordinated updates on both sides.

---

## 8. Error codes (`WorkbenchError.code`)

| Code | Meaning |
|---|---|
| `NOT_FOUND` | Generic "entity does not exist" |
| `ALREADY_EXISTS` | Duplicate insert |
| `ALREADY_REGISTERED` | Project path already registered |
| `NOT_ARCHIVED` | Attempt to unarchive an active project |
| `PATH_NOT_FOUND` | Filesystem path does not exist |
| `PATH_NOT_A_DIRECTORY` | Path exists but isn't a directory |
| `WORKING_DIRECTORY_INVALID` | Session cwd does not exist or is unreadable |
| `PROJECT_ARCHIVED` | Operation not valid on archived project |
| `SPAWN_FAILED` | PTY or process spawn failed; `details.os_error` carries errno |
| `COMPANION_SPAWN_FAILED` | Companion terminal could not be created |
| `WRITE_FAILED` | PTY write returned an error |
| `SESSION_ENDED` | Operation requires a live session |
| `CONTENT_EMPTY` | Note / reminder content rejected |
| `NAME_TAKEN` | Layout name already exists |
| `INTERNAL` | Fallback; `message` carries detail |

---

## 9. Testing contract

For each Tauri command:

- **Contract test** (Rust, `src-tauri/tests/contract/`) verifies: request/response schema, happy-path invariants, every documented error variant, idempotency where applicable.
- **Integration test** (Rust) covers any cross-entity invariants (e.g., `session_activate` creates exactly one companion row).
- **Frontend unit test** (Vitest) stubs the command surface and pins the wire format from the Svelte side.

No Tauri command ships without all three test layers green.
