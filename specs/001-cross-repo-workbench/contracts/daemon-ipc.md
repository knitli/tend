# Daemon IPC Protocol

The workbench runs a small IPC server on a Unix-domain socket so that external processes — primarily the `agentui run` CLI wrapper, optionally cooperating agent integrations — can register sessions and push status into the workbench without needing the user to do anything in the GUI. This satisfies FR-008 (sessions started outside the workbench) for the supported path.

---

## 1. Transport

- **Linux / macOS**: Unix-domain stream socket at `$XDG_RUNTIME_DIR/agentui.sock` (fallback `/tmp/agentui-$UID.sock`).
- **Windows (future)**: Named pipe `\\.\pipe\agentui-$USER`. Out of scope for v1 but the protocol is transport-agnostic.
- **Discovery**: The workbench sets the env var `AGENTUI_SOCKET=<path>` in the environment of any process it spawns. The CLI wrapper reads this. Agents launched outside the workbench can discover it via the same env var if the user has it in their shell config, or by probing the default path.
- **Permissions**: Socket is created with `0600` and the directory must be owned by the same user. No cross-user access.

---

## 2. Framing

- **Length-prefixed JSON**: each message is a little-endian `u32` byte length followed by that many bytes of UTF-8 JSON.
- Client and server speak the same frame format in both directions.
- Max message size: 64 KiB (messages larger than this should not be needed in v1; larger payloads — e.g. bulk event replays — are rejected with `MESSAGE_TOO_LARGE`).

---

## 3. Message shape

Every message is a JSON object with a `kind` field (string, enum) and the rest of the fields specific to that kind.

### Request kinds (client → server)

#### `hello`

Sent first on connection. Advertises client identity and the version it speaks.

```json
{
  "kind": "hello",
  "client": "agentui-run",
  "client_version": "0.1.0",
  "protocol_version": 1
}
```

**Response**: `welcome` (see below) or `protocol_error`.

#### `register_session`

Registers a new session with the workbench. The wrapper calls this once, before starting the agent child. The workbench allocates a session id and returns it.

```json
{
  "kind": "register_session",
  "project_path": "/home/user/marque",
  "label": "parser rewrite",
  "working_directory": "/home/user/marque/crates/marque-parser",
  "command": ["claude", "--model", "sonnet"],
  "pid": 12345,
  "metadata": { "task_title": "Refactor lexer", "branch": "001-lexer" }
}
```

**Required**: `project_path`, `pid`
**Optional**: `label` (defaults to `"session N"`), `working_directory` (defaults to `project_path`), `command`, `metadata`

**Response**: `session_registered { session_id }` or `err`.

Behavior:

- If `project_path` canonicalizes to a known active project, the session is attached to it.
- If `project_path` is unknown, the workbench creates a new project with a default display name (basename of the path) and attaches the session. The user can rename it later.
- If `project_path` does not exist on disk, returns `err { code: "PATH_NOT_FOUND" }`.

#### `update_status`

Pushes a status update for a registered session. The preferred path for cooperating agents.

```json
{
  "kind": "update_status",
  "session_id": 42,
  "status": "needs_input",
  "reason": "Waiting for user to approve file edit",
  "summary": "Editing crates/marque-parser/src/lexer.rs"
}
```

- `status`: `working` \| `idle` \| `needs_input`. `ended` and `error` are derived by the workbench from child exit; clients do not emit them directly.
- `reason`: optional human-readable string displayed with the alert.
- `summary`: optional activity summary string; overrides heuristic derivation for this session until the next `update_status` or a timeout.

**Response**: `ack` or `err`.

#### `emit_alert`

Legacy / explicit path: raise an alert independently of a status change. Primarily useful for agents that want to raise a non-`needs_input` future alert kind without changing session status.

```json
{
  "kind": "emit_alert",
  "session_id": 42,
  "alert_kind": "needs_input",
  "reason": "Permission required"
}
```

v1 treats this identically to an `update_status` with `status = needs_input`. Kept separate for future alert kinds.

**Response**: `ack { alert_id }` or `err`.

#### `heartbeat`

Optional keep-alive. If the workbench hasn't heard from a registered session's client for > 30 s, it falls back to heuristic detection for that session and marks its `status_source` as `heuristic`. Heartbeats reset the clock.

```json
{ "kind": "heartbeat", "session_id": 42 }
```

**Response**: `ack`.

#### `end_session`

Voluntary end notification from the client (e.g., the wrapper observed the child exit and wants to tell the workbench immediately instead of waiting for the reaper).

```json
{ "kind": "end_session", "session_id": 42, "exit_code": 0 }
```

**Response**: `ack`.

### Response kinds (server → client)

#### `welcome`

```json
{
  "kind": "welcome",
  "server_version": "0.1.0",
  "protocol_version": 1,
  "session_id_format": "i64"
}
```

#### `session_registered`

```json
{ "kind": "session_registered", "session_id": 42, "project_id": 7 }
```

#### `ack`

```json
{ "kind": "ack" }
```

With payload variants:

```json
{ "kind": "ack", "alert_id": 13 }
```

#### `err`

```json
{
  "kind": "err",
  "code": "PATH_NOT_FOUND",
  "message": "project_path does not exist on disk",
  "details": { "path": "/home/user/typo-project" }
}
```

Error codes mirror the Tauri-command error codes (`tauri-commands.md` §8) plus:

| Code | Meaning |
|---|---|
| `PROTOCOL_ERROR` | Unknown `kind`, missing required field, or wrong `protocol_version` |
| `MESSAGE_TOO_LARGE` | Frame exceeded 64 KiB |
| `UNAUTHORIZED` | Socket permissions rejected (reserved; v1 only allows same-user) |

---

## 4. Server → client push events (streaming)

After `hello`, the server may push unsolicited messages on the same connection for events the client is subscribed to. In v1 the only subscriber is the workbench GUI itself talking to its own daemon over a process-internal channel — external clients like the CLI wrapper do not subscribe. The protocol reserves the following event kinds for future use:

- `session_status_changed`
- `alert_raised`
- `alert_cleared`

Clients that don't want push events can disconnect after each request/response round trip.

---

## 5. CLI wrapper flow (`agentui run`)

End-to-end call sequence when a user runs `agentui run -p marque -- claude`:

1. CLI parses args, resolves `marque` against registered projects (or uses `$PWD` if unqualified).
2. CLI opens `$AGENTUI_SOCKET`, sends `hello`, receives `welcome`.
3. CLI allocates a PTY locally, `fork+exec`s `claude` inside it.
4. CLI sends `register_session { project_path, pid, command, working_directory }`, receives `session_registered { session_id }`.
5. CLI proxies bytes: stdin → PTY, PTY → stdout. The user sees `claude` as if they ran it directly.
6. CLI monitors PTY output and emits `update_status` when it sees prompt patterns or when it detects `working` / `idle` transitions from byte-level activity. (Same heuristic library the workbench itself uses.)
7. On child exit, CLI sends `end_session { session_id, exit_code }` and exits with the same code.
8. Workbench receives the registration, spawns a `LiveSession` actor that **reattaches** to the running PTY via the registered pid. (Or: in the simpler v1 path, the CLI continues to own the PTY and proxies output to the workbench via a second channel. To be decided in implementation — this is one of the open design choices tracked in `research.md` §6.)

The two paths above differ in who owns the PTY handle after registration. The cleaner answer is "the CLI owns it; the workbench displays output via a secondary stream from the CLI" — keeps the CLI fully independent of the workbench process lifecycle.

---

## 6. Backward / forward compatibility

- `protocol_version` is a single integer. Bumped only on breaking changes.
- New fields in existing message kinds are ignored by older servers and optional on older clients; adding fields is not a version bump.
- New message kinds require a version bump. Clients that request a newer `protocol_version` than the server supports receive `err { code: "PROTOCOL_ERROR" }` on `hello` and must fall back or exit.

---

## 7. Contract tests

Each request kind has a contract test in `src-tauri/tests/contract/daemon/` that:

1. Spins up a temp socket and a mock workbench state.
2. Sends a well-formed frame.
3. Asserts the response shape matches this document.
4. Sends malformed variants (missing fields, wrong types, oversize frames) and asserts the documented error codes.

The CLI wrapper has a matching set of tests in `cli/tests/` that exercise the same happy paths from the client side.

No kind ships without both sides green.
