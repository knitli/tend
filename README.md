# agentui

A local, always-on desktop workbench that unifies AI agent sessions across multiple repositories into a single view with live status, needs-input alerts, one-action activation, paired companion terminals, a per-project scratchpad, and automatic workspace persistence.

Built with **Tauri 2** (Rust backend) + **Svelte 5** (frontend) + **xterm.js** (terminal rendering).

## Quickstart

See the full [developer quickstart](specs/001-cross-repo-workbench/quickstart.md) for detailed instructions.

### Prerequisites

- Rust stable >= 1.80 (`rustup`)
- Node.js 20 LTS+
- pnpm 9.x
- Tauri 2 system deps (WebKitGTK, etc. — see [Tauri prerequisites](https://tauri.app/start/prerequisites/))
- SQLite 3.38+ headers

### Build and run

```bash
pnpm install
cargo fetch
pnpm tauri dev
```

### Run tests

```bash
# Backend (Rust)
cargo test

# Frontend (Svelte + TypeScript)
pnpm check        # type-check
pnpm test         # vitest

# Lints
cargo clippy --workspace -- -D warnings
cargo fmt --check
cargo deny check bans

# E2E (requires tauri-driver)
pnpm e2e
```

## Architecture

```
agentui/
  protocol/       # Shared daemon IPC wire types (serde enums)
  src-tauri/      # Rust backend: PTY management, SQLite, daemon IPC, Tauri commands
  cli/            # CLI wrapper: `agentui run` — launches agents under PTY, registers with workbench
  src/            # Svelte 5 frontend: session list, split view, scratchpad, alerts
  tests/e2e/      # Playwright E2E specs
```

The backend owns all session lifecycles, spawning agents under PTYs via `portable-pty` and running per-session actor tasks on Tokio. Sessions started outside the workbench connect via a length-prefixed JSON daemon IPC protocol over a Unix-domain socket.

For the full specification, plan, and technical decisions, see [`specs/001-cross-repo-workbench/`](specs/001-cross-repo-workbench/).

## Known v1 limitations

- Sessions started without `agentui run` (or without cooperating IPC) will not appear in the workbench.
- Remote sessions (SSH, cloud workstations, WSL-to-Windows) are not supported.
- No VS Code / editor companion — terminal only.
- Scratchpads are plain text with light markdown; no images or attachments.
- Activity summary uses a ring-buffer last-line heuristic; no LLM summarization.
- Workbench-owned sessions become read-only mirrors after a workbench restart (PTY fd does not survive process restart in v1).

Each limitation is tracked as a post-v1 enhancement in [`research.md` section 16](specs/001-cross-repo-workbench/research.md).

## License

Private. Not yet licensed for distribution.
