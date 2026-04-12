# Implementation Plan: Cross-Repo Agent Workbench

**Branch**: `001-cross-repo-workbench` | **Date**: 2026-04-11 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-cross-repo-workbench/spec.md`

## Summary

Build a local, always-on desktop workbench that unifies agent sessions across multiple repositories into a single view with live status, needs-input alerts, one-action activation, paired companion terminals, a per-project scratchpad with a cross-project reminder overview, and automatic workspace persistence.

**Technical approach** (full rationale in [`research.md`](./research.md)): a **Tauri 2** desktop app with a **Rust** backend and a **Svelte 5 + Vite** frontend. The backend owns all session lifecycles — it spawns agents under PTYs via `portable-pty`, runs per-session actor tasks on Tokio for reader/writer/status-monitor duties, and persists projects, sessions, scratchpads, layouts, and workspace state in SQLite via `sqlx`. Sessions started "outside" the workbench are brought in via an `agentui run` CLI wrapper that speaks a small length-prefixed-JSON **daemon IPC protocol** over a Unix-domain socket. The frontend renders one xterm.js pane per agent session and one per companion terminal, organized into a split view on activation. Notifications use Tauri's built-in plugin. Everything is offline and local-only for v1; WSL-to-Windows and remote sessions are explicit non-goals.

## Technical Context

**Language/Version**: Rust stable ≥ 1.80 (backend + CLI); TypeScript 5.x + Svelte 5 runes (frontend)
**Primary Dependencies**: Tauri 2.x, `portable-pty`, `sqlx` (sqlite feature), `tokio`, `notify`, `sysinfo`, `tracing`, `serde` / `serde_json`; xterm.js 5 with `xterm-addon-fit` and `xterm-addon-web-links`, `marked`, `DOMPurify`, Vite
**Storage**: SQLite via `sqlx` at the XDG data directory (`~/.local/share/agentui/workbench.db` on Linux); forward-only migrations under `src-tauri/migrations/`
**Testing**: `cargo test` + `tokio::test` for Rust unit/integration/contract tests; Vitest for Svelte components and stores; Playwright via `tauri-driver` for a small E2E happy-path suite; 80 % backend + CLI coverage gate
**Target Platform**: Linux (X11 + Wayland) primary for v1; macOS and Windows supported as a by-product of Tauri + `portable-pty` but not in v1 CI matrix
**Project Type**: Desktop application (Tauri app) plus a standalone CLI wrapper crate (`agentui run`) and a shared wire-format crate (`agentui-protocol`) that both the Tauri backend and the CLI depend on as the single source of truth for daemon IPC types
**Performance Goals**: 10 concurrent sessions across 5 repos with no perceptible lag; < 50 MB idle RAM; session list / filter updates < 100 ms; session activation < 200 ms; xterm.js rendering at 60 fps
**Constraints**: Local-only (no network I/O in v1); offline-capable; single-user, single-machine; SQLite single-writer; no remote-session support (FR-022)
**Scale/Scope**: 10 sessions × 5 repos concurrent; hundreds of notes/reminders per project over time; archived rows retained indefinitely; < 64 KiB per daemon IPC message

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

The repository's `.specify/memory/constitution.md` is the **unmodified template**. It contains only placeholder principles (`[PRINCIPLE_1_NAME]` / `[PRINCIPLE_1_DESCRIPTION]` etc.) and has not been ratified. There are no project-specific constraints to gate against beyond the global engineering principles already captured in the user's `CLAUDE.md` / `PRINCIPLES.md` / `RULES.md`.

**Gates applied** (drawn from the global rules that are effectively in force for this project):

| Gate | Status | Notes |
|---|---|---|
| Scope discipline — build only what the spec asks | **PASS** | Plan adds no features beyond the spec's 32 FRs. Non-goals (remote sessions, editor integration, rich-media scratchpads, LLM summarization) are listed explicitly in both the spec and `research.md` §16. |
| Test-first, 80 % coverage | **PASS** | Contract + integration + unit tests are the defined test strategy (`research.md` §15). TDD is mandated in the task-level workflow. 80 % coverage gate on Rust and CLI; frontend uses E2E-plus-components as the coverage story. |
| Workspace hygiene, small focused files | **PASS** | The proposed project structure (below) puts each actor/service/UI-concern in its own module. No single file is planned at > 800 LoC; actors and UI components are the natural decomposition boundary. |
| Immutable data flow by default | **PASS** | Rust newtypes + `Clone`-on-write state; Svelte 5 runes give compile-time-checked reactive state without shared-mutable-state footguns. Mutations happen only inside the backend's `WorkbenchState` behind an `RwLock`. |
| Security: no hardcoded secrets, validate inputs at boundaries | **PASS** | No secrets involved. All inputs from the frontend and from the daemon IPC socket are validated in Rust with explicit error variants (contract tests enforce). Socket is `0600` and same-user only. |
| Failure investigation: root-cause over workaround | **PASS** | Errors return structured `WorkbenchError { code, message, details }` values; nothing is silently swallowed. Fallback paths (heuristic status detection) are explicitly labeled as such in the data model (`status_source = heuristic`). |
| Temporal awareness (ISO-8601 timestamps, UTC) | **PASS** | All persisted timestamps are ISO-8601 UTC strings. Age is computed as `max(Duration::ZERO, now_utc - created_at)` at read time (wall-clock diff with a zero floor). Monotonic non-decrease for `last_activity_at` is enforced by a writer-side `max(current, candidate)` clamp, NOT by a paired `std::time::Instant` — see `data-model.md §4` invariant #8 for the full mechanism and its limitations. The earlier draft of this row said "against a monotonic clock at read time", which was imprecise; corrected in round 2. |
| Professional honesty | **PASS** | Known v1 gaps are enumerated rather than hand-waved. Fallback session-state detection is called out as best-effort in the UI, not dressed up as reliable. |

**Result**: **PASS** — no constitution violations; no `Complexity Tracking` entries required.

## Project Structure

### Documentation (this feature)

```text
specs/001-cross-repo-workbench/
├── plan.md              # This file (/speckit.plan output)
├── spec.md              # The feature specification (/speckit.specify output)
├── research.md          # Phase 0 — tech decisions and rationale
├── data-model.md        # Phase 1 — persistent + in-memory entities
├── contracts/
│   ├── README.md        # Contract surface overview
│   ├── tauri-commands.md  # Frontend ↔ backend RPC surface
│   └── daemon-ipc.md    # CLI wrapper ↔ workbench protocol
├── quickstart.md        # Dev bootstrap / exercise guide
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Phase 2 output — /speckit.tasks (NOT created by /speckit.plan)
```

### Source Code (repository root)

Tauri-shaped workspace with a dedicated crate for the backend, a Svelte frontend under a sibling directory, a standalone CLI crate, a tiny shared wire-format crate both Rust crates depend on, and shared integration tests at the root:

```text
agentui/
├── protocol/                      # agentui-protocol — shared daemon IPC types
│   ├── Cargo.toml                 # Tiny: serde + serde_json + thiserror, no tokio/sqlx/tauri
│   └── src/
│       └── lib.rs                 # Request / Response / ErrorCode enums, the wire SOT
│
├── src-tauri/                     # Rust backend (Tauri app)
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── migrations/                # sqlx forward-only migrations
│   │   └── 20260411000001_init.sql
│   └── src/
│       ├── main.rs                # Tauri bootstrap + window wiring
│       ├── commands/              # Tauri command handlers
│       │   ├── mod.rs
│       │   ├── projects.rs        # project_list / register / archive …
│       │   ├── sessions.rs        # session_list / spawn / activate …
│       │   ├── companions.rs
│       │   ├── scratchpad.rs      # notes + reminders + cross-project overview
│       │   ├── workspace.rs       # workspace_get / save, layouts
│       │   └── notifications.rs
│       ├── daemon/                # Unix-domain-socket IPC server
│       │   ├── mod.rs
│       │   ├── server.rs          # Framing + accept-loop; wire types re-exported from `agentui-protocol`
│       │   └── handlers.rs        # No local protocol module — wire SOT lives in `protocol/`
│       ├── session/               # Session lifecycle / actors
│       │   ├── mod.rs
│       │   ├── live.rs            # LiveSession + per-session tasks
│       │   ├── pty.rs             # portable-pty integration
│       │   ├── status.rs          # working / idle / needs_input detection
│       │   └── supervisor.rs
│       ├── companion/             # Companion terminal lifecycle
│       │   └── mod.rs
│       ├── project/               # Project discovery, canonicalization, watching
│       │   ├── mod.rs
│       │   └── watcher.rs         # notify-based filesystem watcher
│       ├── workspace/             # Workspace state save/restore + layouts
│       │   └── mod.rs
│       ├── scratchpad/            # Notes, reminders, overview queries
│       │   └── mod.rs
│       ├── db/                    # sqlx pool + migration runner
│       │   ├── mod.rs
│       │   └── queries.rs
│       ├── model/                 # Domain types (Project, Session, …)
│       │   └── mod.rs
│       ├── notifications/         # Tauri notification plugin glue
│       │   └── mod.rs
│       ├── error.rs               # WorkbenchError + IntoResponse mappings
│       ├── state.rs               # WorkbenchState (Arc<RwLock<…>>)
│       └── lib.rs
│
├── src/                           # Svelte 5 frontend (Vite)
│   ├── app.html
│   ├── app.css
│   ├── main.ts
│   ├── lib/
│   │   ├── api/                   # Tauri invoke() wrappers + event subscriptions
│   │   │   ├── projects.ts
│   │   │   ├── sessions.ts
│   │   │   ├── scratchpad.ts
│   │   │   └── workspace.ts
│   │   ├── stores/                # Reactive state (Svelte 5 runes)
│   │   │   ├── projects.svelte.ts
│   │   │   ├── sessions.svelte.ts
│   │   │   ├── scratchpad.svelte.ts
│   │   │   ├── overview.svelte.ts
│   │   │   └── workspace.svelte.ts
│   │   ├── components/
│   │   │   ├── Sidebar.svelte
│   │   │   ├── SessionList.svelte
│   │   │   ├── SessionRow.svelte
│   │   │   ├── SplitView.svelte           # Agent pane + companion pane
│   │   │   ├── AgentPane.svelte           # xterm.js instance for agent PTY
│   │   │   ├── CompanionPane.svelte       # xterm.js instance for companion shell
│   │   │   ├── AlertBar.svelte
│   │   │   ├── Scratchpad.svelte          # Per-project notes + reminders
│   │   │   ├── CrossProjectOverview.svelte
│   │   │   ├── LayoutSwitcher.svelte
│   │   │   └── SettingsDialog.svelte
│   │   └── util/
│   │       ├── age.ts                     # Reminder age formatting
│   │       └── markdown.ts                # marked + DOMPurify
│   └── routes/
│       └── +page.svelte
│
├── cli/                           # Standalone `agentui run` wrapper crate
│   ├── Cargo.toml                 # Depends on `agentui-protocol` — not on `src-tauri` internals
│   └── src/
│       ├── main.rs
│       ├── args.rs
│       ├── pty.rs                 # Wrapper-side PTY (proxies to child)
│       ├── ipc.rs                 # Daemon IPC client (uses agentui-protocol types)
│       └── heuristic.rs           # Shared status heuristic library
│
├── tests/                         # Cross-cutting integration / E2E tests
│   ├── e2e/                       # Playwright via tauri-driver
│   │   ├── first-run.spec.ts
│   │   ├── session-lifecycle.spec.ts
│   │   ├── scratchpad.spec.ts
│   │   └── workspace-restore.spec.ts
│   └── integration/               # Cross-crate Rust integration tests
│       └── session_to_scratchpad.rs
│
├── Cargo.toml                     # Workspace root (src-tauri + cli)
├── package.json                   # pnpm workspace manifest
├── pnpm-workspace.yaml
├── vite.config.ts
└── tsconfig.json
```

**Structure Decision**: **Tauri desktop app with an adjacent CLI crate and a tiny shared protocol crate**, chosen because:

1. The feature is fundamentally a GUI with a backend daemon role — Tauri gives us both with one process.
2. The CLI wrapper (`agentui run`) is a separate concern with its own dependency graph (no GUI, no webview) and benefits from being its own crate; a Cargo workspace keeps them in one repo without coupling them.
3. **The `agentui-protocol` crate is the single source of truth for the daemon IPC wire format.** Both `src-tauri` and `cli` depend on it via `{ path = "../protocol" }`. Neither crate is allowed to redefine `Request` / `Response` / `ErrorCode` locally. This is cheap to do on day 1 and expensive to retrofit later — editing the wire format means editing exactly one crate, and neither crate imports internals from the other. No path-dep from `cli/` into `src-tauri/src/daemon/` is permitted.
4. The Svelte frontend lives at `src/` per Vite/SvelteKit convention and sits next to `src-tauri/` per Tauri convention — standard layout, no surprises.
5. `tests/` at the workspace root contains only cross-crate tests (E2E and integration across `src-tauri` + `cli`); per-crate unit tests live inside each crate per Rust convention.
6. Migrations live next to the backend crate (`src-tauri/migrations/`) so they ship with the binary that runs them.

## Phase 0 outputs

Written to [`research.md`](./research.md). Every open technical question from Technical Context is resolved there. No unresolved `NEEDS CLARIFICATION` markers.

Summary of decisions (full rationale in research.md):

1. Tauri 2 over Electron and native Rust UI (§1)
2. Svelte 5 + Vite over React/Solid (§2)
3. xterm.js 5 as the only mature browser terminal emulator (§3)
4. `portable-pty` for cross-platform PTY spawning (§4)
5. SQLite via async `sqlx` for persistence, XDG data dir, forward-only migrations (§5)
6. Workbench-owned sessions via `agentui run` CLI wrapper + daemon IPC for session discovery (§6)
7. Two-tier session state detection: cooperative IPC primary, output-activity + prompt heuristic fallback (§7)
8. Intra-app view switching for "bring to foreground" — no cross-process window focus (§8)
9. Tauri notification plugin for OS-native alerts; in-app alert bar always visible (§9)
10. Canonical absolute path as project identity; worktrees opt-in (§10)
11. Separate tables for auto-saved Workspace State and user-named Layouts (§11)
12. SQLite rows for notes/reminders; plain text + light markdown; FTS5 deferred until needed (§12)
13. Ring-buffer last-line activity summary, with agent-provided override (§13)
14. Actor-per-session on Tokio multi-threaded runtime (§14)
15. Contract + integration + unit tests with 80 % backend + CLI coverage (§15)

## Phase 1 outputs

- [`data-model.md`](./data-model.md) — 9 SQLite tables (`projects`, `sessions`, `companion_terminals`, `notes`, `reminders`, `workspace_state`, `layouts`, `notification_preferences`, `alerts`), in-memory `LiveSession` actor handle, derived views, invariants, migration plan.
- [`contracts/README.md`](./contracts/README.md) — surface overview.
- [`contracts/tauri-commands.md`](./contracts/tauri-commands.md) — 32 Tauri commands across 7 domains (projects, sessions, companions, scratchpad, workspace/layouts, notifications, events), plus error code catalog and test layering.
- [`contracts/daemon-ipc.md`](./contracts/daemon-ipc.md) — framing, message shapes (`hello`, `register_session`, `update_status`, `heartbeat`, `end_session` — `emit_alert` was dropped from v1 per the round-2 spec-panel review), responses, error codes, CLI wrapper flow, compatibility rules.
- [`quickstart.md`](./quickstart.md) — dev bootstrap, first build, first project, first session, test commands, troubleshooting, known v1 limitations.
- **Agent context file** — updated via `.specify/scripts/bash/update-agent-context.sh claude` (see Phase 1 closing step).

## Post-Design Constitution Re-check

Re-evaluating the gates against the completed Phase 1 design:

| Gate | Phase 1 status | Notes |
|---|---|---|
| Scope discipline | **PASS** | The 32 Tauri commands and 6 daemon IPC verbs each map to at least one FR in `spec.md`. No orphan features. |
| Test-first | **PASS** | Contracts specify the test layering explicitly: every command has contract + integration + frontend-unit tests before implementation. |
| Small focused files | **PASS** | Directory breakdown maps one concern per file; no module is planned > 500 LoC. |
| Immutable data flow | **PASS** | Backend types are `Clone`-friendly newtypes; mutations funneled through `WorkbenchState`. Frontend stores use Svelte 5 runes with derived state. |
| Security / input validation | **PASS** | Every Tauri command and daemon verb lists its error codes in the contract; contract tests enforce them. Socket permissions documented. |
| Failure investigation | **PASS** | `status_source = heuristic` tracks fallback detection; crash recovery on workbench startup runs a single-pass `reconcile_and_reattach` (T025) — reattaching live pids as attached-mirror handles and marking dead pids `ended` in one transaction — eliminating a latent ordering bug flagged in the spec-panel review. T025b is the integration gate that enforces this invariant. |
| Temporal awareness | **PASS** | Timestamps are ISO-8601 UTC throughout data model; age is derived, not stored. |
| Professional honesty | **PASS** | Known v1 gaps list in research.md §16 matches the quickstart's "Known v1 limitations" section verbatim in intent. |

**Result**: **PASS** — no new violations introduced by Phase 1 design; ready to proceed to `/speckit.tasks`.

## Complexity Tracking

*Only filled when gate violations require justification. This plan has no violations to justify.*

No entries.
