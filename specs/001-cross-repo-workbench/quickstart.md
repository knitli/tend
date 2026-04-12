# Quickstart: Cross-Repo Agent Workbench (Dev)

This document walks a developer through bootstrapping, building, running, and exercising the Cross-Repo Agent Workbench during development. It is **not** end-user documentation — it assumes you have the repo cloned and a working toolchain.

---

## 1. Prerequisites

Install once:

- **Rust** stable ≥ 1.80 via `rustup` (with components `rustfmt`, `clippy`)
- **Node.js** 20 LTS or newer
- **pnpm** 9.x (`npm install -g pnpm`) — used for the frontend workspace
- **Tauri 2 system deps** — Linux: `webkit2gtk-4.1`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libgtk-3-dev`, plus the usual `build-essential` and `pkg-config`. See <https://tauri.app/start/prerequisites/>.
- **SQLite** 3.38+ headers (usually pulled in transitively, but install `libsqlite3-dev` on Debian/Ubuntu if `sqlx` complains)
- Optional: **`tauri-driver`** (`cargo install tauri-driver`) and a WebDriver binary — needed only for E2E tests

Verify:

```bash
rustc --version
node --version
pnpm --version
pkg-config --list-all | grep -E 'webkit2gtk|gtk\+-3'
```

---

## 2. First build

```bash
# from repo root
pnpm install          # installs frontend deps in ./src and workspace root
cargo fetch           # warms the Rust cache

# build the backend (src-tauri) and frontend together
pnpm tauri dev
```

First run takes several minutes — `tauri dev` compiles the Rust backend, spins up Vite in watch mode for the Svelte frontend, and opens the workbench window once both are ready. Subsequent runs are cached.

A successful first run should:

1. Create `~/.local/share/agentui/workbench.db` (Linux) and run initial migrations.
2. Create the IPC socket at `$XDG_RUNTIME_DIR/agentui.sock` (fallback `/tmp/agentui-$UID.sock`).
3. Log `workbench ready` to stdout.
4. Open the main window to an empty "No projects registered" state.

---

## 3. Register your first project

From the GUI: click **Add Project** and pick a directory, or drag a repo onto the workbench window.

From the CLI (coming later in the task sequence):

```bash
# Explicit registration
agentuictl project add ~/code/marque --name "Marque"
```

Confirm it appears in the sidebar. The project row in SQLite should have:

```sql
SELECT id, display_name, canonical_path FROM projects;
-- 1 | Marque | /home/you/code/marque
```

---

## 4. Run your first session

The v1 session path is the CLI wrapper. From a terminal:

```bash
# AGENTUI_SOCKET is exported automatically if the workbench is running
agentui run -p marque -- claude
```

What happens:

1. The wrapper opens a PTY and starts `claude` inside it.
2. It registers with the workbench over the daemon IPC (you'll see the session appear in the sidebar almost instantly).
3. Your terminal is pretty much unchanged — `claude` runs as normal and you interact with it directly.
4. In the workbench, activating this session shows a split view:
   - **Left pane**: mirrored view of the agent's PTY output (read-only from the workbench's side; input still comes from your terminal).
   - **Right pane**: a **companion terminal** already `cd`'d into `~/code/marque`, ready for ad-hoc commands.

Start a second session in a different repo:

```bash
cd ~/code/agentui
agentui run -p agentui -- claude
```

You now have two sessions in the workbench, grouped by project. Switching between them via the workbench sidebar brings up each one's split view (agent + companion terminal) without you alt-tabbing through terminals.

---

## 5. Try the core flows

### 5.1 "Needs input" alert

Start a session that will prompt you. Easiest reproducible example:

```bash
agentui run -p marque -- bash -c 'read -p "continue? [y/N] " x; echo you said: $x'
```

Within a second or two the workbench should mark that session `needs_input` and raise an OS notification. Typing `y` in your actual terminal clears the alert automatically.

### 5.2 Scratchpad note

In the workbench, open a project and add a note in the Scratchpad panel: `"lexer rewrite blocks on tokenizer refactor"`. Close the workbench. Reopen it. The note is still there.

### 5.3 Reminder

Add a reminder: `"ask Knitli about the grammar conflict in rule 37"`. Check the Cross-Project Overview — the reminder appears grouped under its project. Mark it done. It vanishes from the overview but is still retrievable from the project's scratchpad history.

### 5.4 Workspace restore

Close the workbench entirely. Reopen it. Your projects are still registered, your last active session is re-selected, and any still-running sessions (e.g., long-lived `claude` processes you didn't kill) reattach automatically.

---

## 6. Running tests

### Backend (Rust)

```bash
cargo test                # all unit + integration tests
cargo test --test contract # Tauri command + daemon IPC contract tests
cargo tarpaulin --out Html # coverage (target ≥ 80 %)
```

### Frontend (Svelte + TS)

```bash
pnpm --filter frontend test         # Vitest
pnpm --filter frontend test:coverage
```

### End-to-end (Tauri app)

```bash
pnpm --filter frontend e2e          # Playwright via tauri-driver
```

E2E tests require `tauri-driver` installed and a WebDriver binary on PATH.

---

## 7. Database tinkering

The SQLite file is at `~/.local/share/agentui/workbench.db`. It's a plain SQLite 3 database — inspect with `sqlite3` or any GUI (DB Browser, TablePlus, etc.):

```bash
sqlite3 ~/.local/share/agentui/workbench.db '.tables'
sqlite3 ~/.local/share/agentui/workbench.db 'SELECT status, COUNT(*) FROM sessions GROUP BY status;'
```

To reset completely:

```bash
# stop the workbench first
rm ~/.local/share/agentui/workbench.db
```

Migrations re-run on next launch.

---

## 8. Troubleshooting

**Workbench window doesn't open**: check `pnpm tauri dev` stdout for a WebKitGTK error. Reinstall the system deps in §1.

**`sqlx` compile errors about SQLITE_VERSION**: upgrade `libsqlite3-dev` to 3.38+ or enable the bundled feature in `src-tauri/Cargo.toml`.

**`agentui run` says "socket not found"**: the workbench isn't running, or `AGENTUI_SOCKET` isn't exported in your shell. Start the workbench, then re-source your shell's env (or explicitly `export AGENTUI_SOCKET=$XDG_RUNTIME_DIR/agentui.sock`).

**Session stuck on `working` with no output**: almost always a PTY flush issue. Run `ps -o pid,stat,comm -p <pid>` — if it's in `S+` (TTY-foreground sleep) the agent is genuinely waiting on input; otherwise investigate the session supervisor.

**Companion terminal opens in the wrong directory**: check `sessions.working_directory` in the DB. Worktrees and submodules use the session's actual cwd, not the project root — this is intentional.

### Dev troubleshooting

**`cargo test` fails with "address already in use"**: multiple test binaries may try to create the daemon socket concurrently. The test helpers use unique socket paths via `temp_socket_path()`, but if you run `cargo test -j1` you can serialize to debug.

**Tauri window blank / white screen**: Vite dev server may not have started yet. Check the terminal for `VITE v5.x.x ready` before interacting with the window. On WSL2, ensure `DISPLAY` or `WAYLAND_DISPLAY` is set.

**`cargo deny` fails on a new dependency**: check `deny.toml` for the FR-022 ban list. If a transitive dep pulls in a banned networking crate, you'll need to either find an alternative or add a documented exception with a `[features]` flag.

**Tests flaky on CI / slow machine**: the performance sanity tests (T144) use 200ms thresholds for debug builds. If you're seeing marginal failures, check system load. The production targets (< 100ms) apply to release builds.

**E2E tests won't run**: they require `tauri-driver` (`cargo install tauri-driver`), a debug build of the app (`pnpm tauri build --debug`), and a windowed environment (X11/Wayland). WSL2 without a display server will not work for E2E.

### Performance check

Results from T144 sanity pass (debug build, in-memory SQLite):

| Metric | Target | Result |
|--------|--------|--------|
| `session_list` (10 sessions, 5 projects) | < 100 ms | < 5 ms |
| `session_list` with project filter | < 100 ms | < 2 ms |
| `note_list` (page 1 of 5,000 notes) | < 100 ms | < 10 ms |
| `cross_project_overview` (5,000 notes + 5,000 reminders) | < 100 ms | ~100 ms (debug) |

---

## 9. Known v1 limitations

- Sessions started without `agentui run` (or without cooperating IPC) will not appear in the workbench.
- Remote sessions (SSH, WSL-to-Windows) are not supported.
- No VS Code / editor companion — terminal only.
- Scratchpads are plain text with light markdown; no images or attachments.
- No cloud sync or multi-machine state.

Each of these is tracked as a post-v1 enhancement in `research.md` §16.
