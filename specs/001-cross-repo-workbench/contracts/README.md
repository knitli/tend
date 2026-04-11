# Contracts

The workbench exposes two interface surfaces to the outside world:

1. **Tauri command surface** — the in-process RPC boundary between the Svelte frontend and the Rust backend. See [`tauri-commands.md`](./tauri-commands.md).
2. **Daemon IPC protocol** — the Unix-domain-socket protocol used by the `agentui run` CLI wrapper (and future cooperating agents) to register sessions and push status updates into a running workbench. See [`daemon-ipc.md`](./daemon-ipc.md).

There are no public HTTP endpoints, no GraphQL, and no networked protocols in v1 — the workbench is local-only per FR-022.

Every named command/verb below has a contract test in `src-tauri/tests/contract/` that pins its shape (field names, required fields, error variants). Contract tests MUST be written before the implementation (TDD).
