# Phase 0 Research: Cross-Repo Agent Workbench

**Feature**: `001-cross-repo-workbench`
**Date**: 2026-04-11
**Status**: Complete

This document resolves technical unknowns for the Cross-Repo Agent Workbench before Phase 1 design. Each section follows **Decision / Rationale / Alternatives considered**.

---

## 1. Application shell: native vs Electron vs Tauri

**Decision**: **Tauri 2.x** (Rust backend + web frontend).

**Rationale**:

- The workbench is intended to be always-on. Electron's ~200 MB idle footprint is a real cost for something the user leaves running across a full workday; Tauri's native WebView + Rust backend keeps idle memory under ~50 MB and idle CPU near zero.
- Rust is a strong fit for the backend concerns (PTY spawning, process introspection, async I/O, SQLite, filesystem watching) and has mature crates for every capability the spec needs.
- Tauri 2 ships first-class cross-platform APIs for window management, notifications, system tray, secure file-system access, and local IPC — all of which we need.
- Cross-platform posture matters even though v1 is local-only: the spec's non-goals explicitly acknowledge WSL-to-Windows as a future direction. A Rust core keeps that door open without forcing us to invest in it now.
- We avoid Electron's habit of encouraging "ship a full Chromium per app" for a workbench that's structurally closer to a system utility than a content-heavy app.

**Alternatives considered**:

- **Electron + TypeScript**: Most familiar ecosystem, fastest prototyping. Rejected primarily on always-on footprint and because we'd still end up shelling out to native tools for PTY + process management, which cedes the main benefit.
- **Pure native Rust (egui / iced)**: Smallest footprint, no web layer. Rejected because terminal rendering (xterm.js) is the dominant UI concern and there is no pure-Rust terminal emulator widget with xterm.js-class maturity.
- **Go + Wails / Fyne**: Mid-weight. Rejected — smaller ecosystem than Rust for PTY, weaker async story for our concurrency needs, and less established Tauri-equivalent story.
- **Web app via localhost**: Rejected — no OS notifications, no native window focus, no system-tray presence, awkward cold-start.

---

## 2. Frontend framework

**Decision**: **Svelte 5 (runes) + Vite**, served inside the Tauri WebView.

**Rationale**:

- Compile-time reactivity produces small bundles and keeps the always-on UI cheap.
- Svelte's stores map cleanly onto Tauri events (IPC → store → reactive UI).
- Runes give us a straightforward way to model streaming session status updates without reducers/middleware ceremony.
- We're not building a content site — we're building a dense, live dashboard. Svelte is a good fit for that shape, and Solid/Svelte both beat React for per-frame-update UIs (xterm output + session list rerenders).

**Alternatives considered**:

- **React + Vite**: More ecosystem, more familiar. Rejected because the workbench is update-heavy and we'd end up fighting reconciliation cost without obvious gain.
- **SolidJS**: Technically comparable to Svelte 5 runes. Rejected on smaller ecosystem (fewer component libraries; thinner xterm.js integration examples).
- **Plain TS + lit-html or vanilla**: Rejected — premature minimalism for a multi-view app.

---

## 3. Terminal rendering in the UI

**Decision**: **xterm.js 5.x** with `xterm-addon-fit`, `xterm-addon-web-links`, and (optionally) `xterm-addon-search`, rendered in Svelte components. Each session's agent PTY and each companion terminal gets its own xterm.js instance.

**Rationale**:

- xterm.js is the only mature, actively maintained browser terminal emulator. It powers VS Code's integrated terminal and most Electron-based terminal apps. There is no credible alternative for the WebView path.
- Addons cover the practical needs: fit-to-container, clickable links, in-terminal search.
- Renders at 60 fps via WebGL (or canvas fallback) with tens of thousands of scrollback lines comfortably within our 10×5 scale target.

**Alternatives considered**:

- **Embed an external terminal emulator (Kitty, WezTerm, Alacritty) and steal its window**: Rejected — brittle per-WM focus hackery, poor UX, no cross-platform story.
- **Roll our own canvas terminal**: Rejected — years of work to match xterm.js parity.

---

## 4. PTY spawning on the backend

**Decision**: **`portable-pty` crate** (from the WezTerm project) for PTY allocation, backed by Tokio for async I/O via `async-channel` bridges.

**Rationale**:

- Cross-platform (Unix pty, Windows ConPTY) with a small, battle-tested API — used by WezTerm itself in production.
- Integrates cleanly with Tokio: we read/write the PTY's `Read` + `Write` handles on blocking worker threads and forward bytes across async channels to the Tauri event bridge.
- Supports resize, signal delivery, and child-exit detection — all things we need for companion terminal lifecycle.

**Alternatives considered**:

- **`tokio-pty-process`**: Older, less maintained. Rejected.
- **`rustix-openpty` + manual fork/exec**: Lower-level; we'd end up rebuilding `portable-pty`'s cross-platform abstractions. Rejected.
- **Shell out to `script(1)` / `socat`**: Rejected — platform-specific, fragile, hides exit codes.

---

## 5. Local persistence

**Decision**: **SQLite** via **`sqlx`** (async, compile-time-checked queries) with `sqlite` feature, stored under the XDG data directory:

- Linux: `$XDG_DATA_HOME/agentui/workbench.db` (default `~/.local/share/agentui/workbench.db`)
- macOS: `~/Library/Application Support/agentui/workbench.db`
- Windows: `%APPDATA%/agentui/workbench.db`

Migrations handled by `sqlx migrate` with versioned SQL files under `src-tauri/migrations/`.

**Rationale**:

- The data model includes multiple related entities (Project ↔ Session ↔ Companion Terminal, Project ↔ Note / Reminder, Layouts, Workspace State) with cross-entity queries ("give me all open reminders grouped by project, ordered by age"). SQLite is the right shape; a JSON blob store would make those queries painful and risk sync bugs.
- `sqlx` gives us async-first, compile-time query verification, and straightforward migrations — all valuable for a single-writer desktop app.
- SQLite FTS5 is available if we later need full-text search over long scratchpads (see §12).

**Alternatives considered**:

- **Sled / redb (pure-Rust KV stores)**: Rejected — we need relational queries, and writing a schema layer on top of KV trades short-term wins for long-term pain.
- **JSON files per entity**: Rejected — concurrent-write hazards, no transactions, no indices, poor scratchpad search story.
- **`rusqlite` (sync)**: Acceptable fallback but mixing sync DB with Tokio is awkward. Preferred `sqlx` for async uniformity.

---

## 6. Session discovery: how sessions get into the workbench

**Decision**: **Workbench-owned sessions by default, with a `agentui run` CLI wrapper** that registers any session started from a terminal. A Unix-domain-socket (named-pipe on Windows) **daemon IPC** exposes a tiny protocol so the wrapper — and future agent integrations — can register sessions, push status, and emit "needs input" events.

This resolves FR-008 ("sessions started outside the workbench"): from the user's point of view the session is started from a plain terminal command, but the wrapper pipes it under the workbench's PTY and registers it over the socket. Sessions started without the wrapper are a known limitation in v1, surfaced clearly in the quickstart and mentioned as a known gap below.

**Protocol sketch** (full contract in `contracts/daemon-ipc.md`):

- Socket path: `$XDG_RUNTIME_DIR/agentui.sock` (fallback `/tmp/agentui-$UID.sock`)
- Advertised via env var `AGENTUI_SOCKET` for child processes
- Message framing: length-prefixed JSON (line-delimited JSON as fallback)
- Request verbs: `register_session`, `update_status`, `emit_alert`, `heartbeat`, `end_session`
- Server responses: `ack { session_id }` / `err { code, message }`

**Rationale**:

- Embedding owned sessions sidesteps the entire "focus an external window" rabbit hole. "Activating a session" means switching the visible split in the workbench — no cross-process focus API, no per-WM integration.
- The CLI wrapper path is ergonomic: `agentui run -p marque -- claude` feels like a terminal command, which matches how users already start agents.
- The IPC protocol also lets us support agent-native integrations later: a cooperating agent can speak it directly without the wrapper, improving "needs input" fidelity without architectural change.
- v1 known gap (agents started with no wrapper and no IPC) is explicitly limited — the product value comes from the 95% case where users launch via the wrapper.

**Alternatives considered**:

- **Process scanning** (`ps` / `sysinfo` to find "claude" etc.): Rejected — fragile name matching, no way to infer project context or state, and would need to be rewritten per agent.
- **Filesystem watcher on a "sessions" directory**: Rejected — inventing a file-based protocol that's strictly weaker than a socket.
- **Pure GUI-launched sessions**: Rejected — the user specifically wants to launch agents from terminals as they do today.

---

## 7. Session state detection: working / idle / needs-input

**Decision**: **Two-tier detection**:

- **Tier 1 (preferred): cooperative IPC events.** Agents (or the wrapper) send `update_status` over the daemon socket. This gives us exact, low-latency state transitions including `needs_input` with a reason string.
- **Tier 2 (fallback): output-activity heuristic + prompt-pattern detection.** For sessions that don't emit IPC events:
  - `working` = PTY produced output within the last 2 seconds
  - `idle` = no output for ≥ 5 seconds *and* the last line of the terminal buffer matches a shell prompt pattern (configurable; defaults cover `bash/zsh/fish` prompt endings `$ `, `% `, `> `, `# `)
  - `needs_input` = heuristic match against a small regex library of known agent prompt forms (`[y/N]`, `? `, `Please choose`, `Do you want`, trailing `: ` on an otherwise-idle buffer) — **best-effort**, documented as fallback only

**Rationale**:

- Tier 1 gives us the experience the user actually wants (crisp alerts, no false positives).
- Tier 2 keeps the workbench useful for non-cooperating agents without being load-bearing. We do not want to pretend our heuristic is perfect — we'll flag fallback-sourced "needs input" events visually as "best-effort" so the user isn't misled.
- Matching shell prompts avoids the worst failure mode of treating a session as "needs input" every time output pauses.

**Alternatives considered**:

- **`/proc/<pid>/status` TTY-block detection**: Linux-only, brittle, misses the actual semantic event (the agent may be computing, not blocked). Rejected as primary.
- **Agent log file tailing**: Per-agent-specific; doesn't generalize. Rejected.
- **Pure IPC, no fallback**: Rejected — makes the fallback-path agents second-class citizens, hurting FR-008.

---

## 8. "Bring to foreground" semantics

**Decision**: Because the workbench renders sessions inside its own WebView (xterm.js panes), "activating a session" is an **intra-app view-switch** — the workbench selects that session, swaps the split view to show its agent pane + its companion terminal pane, and raises the workbench's own OS window via Tauri's window API if it's backgrounded.

No cross-process window focus integration is needed for v1. This dramatically simplifies OS integration and eliminates a class of "we couldn't find your terminal window" bugs.

**Rationale**:

- Sessions are already embedded; there's no external window to raise.
- Tauri's `Window::set_focus()` / `Window::show()` handle raising the workbench itself reliably on Linux (X11 + Wayland), macOS, and Windows.
- The cost is that v1 can't focus an externally-hosted terminal — acceptable because the entire v1 architecture is "workbench owns the session."

**Alternatives considered**:

- **Per-WM integration (wmctrl, xdotool, AppleScript, UIA)**: Rejected for v1 — unnecessary given the embedded model, and the complexity dwarfs the feature.

---

## 9. Notifications ("needs input" surfacing)

**Decision**: **Tauri's `tauri-plugin-notification`** for OS-native notifications on Linux, macOS, and Windows. In-workbench alerts are rendered as a pinned alert bar showing every session currently in `needs_input`. Notification channel (OS / in-app / silent-but-visible / terminal bell) is configurable per-project and globally per FR-013.

Quiet-hours: stored as a time range per preference; during quiet hours the workbench suppresses the OS-level channel but still shows the in-app alert, honoring FR-013 and SC-007.

**Rationale**:

- Built-in plugin covers all three platforms without extra deps.
- Multi-channel delivery lets users tune noise vs. awareness.

**Alternatives considered**:

- **`notify-rust` directly**: Viable, but Tauri's plugin already wraps it and handles permissions/capabilities.
- **Email / push**: Out of scope — local-only per FR-022.

---

## 10. Project identity and repository handling

**Decision**: A project's stable identity is the **canonical absolute path of its root directory** (resolved via `std::fs::canonicalize`) plus a user-editable display name. The database stores both; canonical path is the primary key for lookups, display name is presentation-only.

Worktrees: each git worktree is its own project *only if the user explicitly registers it*. Otherwise a session running inside a worktree under a registered parent project is treated as belonging to the parent and the companion terminal opens in the worktree directory (not the parent root).

Renames / moves: a filesystem watcher on registered project roots (via the `notify` crate) raises a `project_path_missing` event when the directory disappears; the workbench marks the project "unavailable" until the user reconciles it (relocate, archive, or remove).

**Rationale**:

- Canonical path disambiguates same-basename repos automatically, covering the edge case in the spec.
- User-editable display names keep the UI readable without losing disambiguation.
- Making worktrees opt-in avoids surprising users whose everyday workflow is one agent session per worktree.

**Alternatives considered**:

- **Git remote URL as identity**: Rejected — breaks for local-only repos, monorepos, and repos without remotes.
- **Inode-based identity**: Rejected — unstable across filesystems and backup/restore.

---

## 11. Workspace state vs named layouts

**Decision**: Two separate SQLite rows/tables:

- `workspace_state` — single-row (or single-key) blob of "what was up the last time the workbench exited." Auto-saved on graceful shutdown and on a 30-second debounced interval during use. Auto-restored on launch, no user action required. Satisfies FR-019, FR-020, SC-005.
- `layouts` — user-named, multi-row. Saved explicitly by the user. Restored by explicit user action. Satisfies FR-018 and User Story 6.

**Rationale**:

- The distinction is important: workspace state is "where was I five minutes ago," layouts are "the client-X work context I switch to twice a week." Collapsing them into one concept would force every user to name their last-session state, which defeats the point.

**Alternatives considered**:

- **Treat last session as an implicit unnamed layout**: Rejected — same outcome in more confusing language.

---

## 12. Scratchpad storage and rendering

**Decision**:

- Notes and reminders are stored as rows in SQLite.
- `notes.content` is plain text (UTF-8), with optional light markdown rendered in the UI via `marked` + `DOMPurify` (no code execution; sanitized output).
- `reminders.state` is an enum (`open` / `done`), with `created_at` used to compute the age indicator (FR-031).
- Search across scratchpads is initially linear (we're capped at ~10 × 5 projects and ~hundreds of rows). If a user's scratchpads grow beyond that we enable **SQLite FTS5** on `notes.content` and `reminders.content` behind a config flag — research tracks this as a known future path, not a v1 requirement.
- Removed projects: rows in `projects` get an `archived_at` timestamp instead of `DELETE`, and child notes/reminders follow the archived project (FR-032).

**Rationale**:

- SQLite rows give us clean cross-project queries ("all open reminders across all projects grouped by project, sorted by age") that are the whole point of the Cross-Project Overview (FR-028).
- Plain text + light markdown matches the spec's "scratchpad, not a Notion clone" framing.
- Archival-not-deletion respects FR-032 and prevents data loss from accidental project removal.

**Alternatives considered**:

- **Rich text / WYSIWYG**: Rejected — explicitly out of scope per the assumptions.
- **Per-project text files on disk**: Rejected — kills cross-project queries and duplicates archival logic.

---

## 13. Session activity summaries (FR-011, User Story 4)

**Decision**: Each session has a ring-buffer of the most recent N (default 200) output lines in memory on the backend. The "activity summary" shown in the overview is the last non-blank, non-prompt line truncated to ~80 characters. Agents that send IPC `update_status` events may also supply a structured `summary` / `task_title` string, which takes precedence over the ring-buffer derivation when present.

**Rationale**:

- Dirt-simple, works with zero agent cooperation, gracefully improves when agents do cooperate.
- Ring buffer is tiny per session (~16 KB) and contained — no risk of blowing memory on a session that vomits output.
- Matches the spec's explicit boundary: this is ephemeral, session-scoped activity (US4), not the persistent user-owned scratchpad (US5).

**Alternatives considered**:

- **Store summaries in SQLite**: Rejected — ephemeral data, churns fast, no value after session ends.
- **Run summaries through an LLM**: Rejected — adds a dependency, cost, and latency for no commensurate v1 value.

---

## 14. Concurrency model (backend)

**Decision**: Single Tokio multi-threaded runtime inside `src-tauri`. Per-session actors:

- A **PTY reader task** that drains the PTY → broadcast channel (status + output events).
- A **PTY writer task** (stdin from frontend → PTY).
- A **status monitor task** that evaluates idle/working/needs-input from the broadcast stream.
- A **supervisor task** per session that owns the child process and cleans up on exit.

Tauri commands dispatch into a single `WorkbenchState` (wrapped in `Arc<tokio::sync::RwLock<_>>`). Database access goes through a `sqlx::SqlitePool`.

**Rationale**:

- Each session's lifecycle is independent; actor-per-session isolates failures and makes ending a session a single `drop`.
- Tokio's cooperative runtime handles 10-50 concurrent session I/O streams trivially.
- `RwLock` on shared state is fine at our scale (state mutations are rare compared to reads).

**Alternatives considered**:

- **Threaded, non-async**: Rejected — PTY I/O multiplexing is fundamentally async-friendly.
- **Actor framework (xtra, riker)**: Rejected — overkill; hand-rolled Tokio tasks are clearer at this scale.

---

## 15. Testing strategy

**Decision**:

- **Rust backend**: `cargo test` with `tokio::test` for async units. Contract tests for every Tauri command and every daemon IPC verb. Integration tests for session lifecycle (spawn → output → idle → needs-input → end) using a scripted shell as the child process.
- **Frontend**: **Vitest** for component logic and Svelte stores; **Playwright via Tauri's `tauri-driver`** for a small suite of end-to-end happy paths (launch, register project, activate session, save note, restore workspace).
- **CLI wrapper (`agentui run`)**: integration tests against a local daemon socket.
- **Coverage target**: 80 % across backend + CLI (aligned with the user's global testing rules). Frontend coverage is measured but not gated at the same threshold — terminal-rendering UI is validated in E2E, not unit, tests.

**Rationale**:

- Matches the user's global testing rules (80 %, TDD, separate unit / integration / E2E).
- Scripted-shell fixtures are the cleanest way to exercise the session state machine deterministically.

**Alternatives considered**:

- **Only manual testing**: Rejected — session state detection is exactly the kind of thing that rots silently without tests.

---

## 16. Known v1 gaps (documented, not blocking)

The following are explicitly deferred to post-v1 based on the spec's out-of-scope markers and this research:

- **Remote sessions** (SSH, cloud workstations, WSL↔Windows cross-boundary) — FR-022.
- **Non-wrapper-launched sessions** — §6 above. Sessions started with no `agentui run` wrapper and no cooperating IPC client will not appear in the workbench. Documented in the quickstart as a known limitation.
- **VS Code / editor companion pairing** — terminal-only for v1 per spec assumption.
- **Rich-media scratchpads** (images, attachments) — plain text + light markdown only.
- **LLM-summarized session activity** — ring-buffer last-line only.
- **FTS5 full-text scratchpad search** — only enabled if scale demands it.

Each gap has a corresponding issue to open once the v1 code lands, not a blocker for `/speckit.plan` → `/speckit.tasks` → `/speckit.implement`.

---

## Summary of technical context values for `plan.md`

- **Language/Version**: Rust (stable, ≥ 1.80) for backend; TypeScript 5.x + Svelte 5 for frontend.
- **Primary Dependencies**: Tauri 2.x, `portable-pty`, `sqlx` (SQLite), `tokio`, `notify`, `sysinfo`, `tracing`; `xterm.js` 5, `marked`, `DOMPurify`, Vite.
- **Storage**: SQLite via `sqlx`, at XDG data dir.
- **Testing**: `cargo test` + `tokio::test` for Rust; Vitest + Playwright (`tauri-driver`) for frontend and E2E.
- **Target Platform**: Linux (X11 + Wayland) primary for v1; macOS and Windows supported as a by-product of Tauri + `portable-pty` but not in v1 CI matrix.
- **Project Type**: Desktop application (Tauri app) + standalone CLI wrapper (`agentui run`).
- **Performance Goals**: 10 concurrent sessions across 5 repos with no perceptible lag; < 50 MB idle RAM; list updates < 100 ms; activation < 200 ms.
- **Constraints**: Local-only (no network I/O for v1); offline-capable; single-user single-machine; SQLite single-writer is acceptable.
- **Scale/Scope**: 10 sessions × 5 repos concurrent; hundreds of notes/reminders per project over time; archival retained indefinitely.
