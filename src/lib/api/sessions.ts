// T060: typed wrappers for session Tauri commands and event subscribers.
// Mirrors the session surface from contracts/tauri-commands.md §2 and the
// event map from events.ts.

import type { UnlistenFn } from "@tauri-apps/api/event";
import {
	listen,
	type SessionEndedEvent,
	type SessionOutputEvent,
	type SessionSpawnedEvent,
} from "./events";
import { invoke } from "./invoke";

// ---------- Types ----------

export type SessionStatus =
	| "working"
	| "idle"
	| "needs_input"
	| "ended"
	| "error";
export type SessionOwnership = "workbench" | "wrapper";

export interface SessionMetadata {
	readonly task_title?: string;
	readonly branch?: string;
	readonly model?: string;
	readonly command?: string[];
	readonly [key: string]: unknown;
}

export interface Alert {
	readonly id: number;
	readonly session_id: number;
	readonly project_id: number;
	readonly reason?: string;
	readonly raised_at: string;
}

/**
 * Full session row as returned by session_spawn and session_end.
 */
export interface Session {
	readonly id: number;
	readonly project_id: number;
	readonly label: string;
	readonly pid: number | null;
	readonly status: SessionStatus;
	readonly status_source: "ipc" | "heuristic";
	readonly ownership: SessionOwnership;
	readonly started_at: string;
	readonly ended_at: string | null;
	readonly last_activity_at: string;
	readonly last_heartbeat_at: string | null;
	readonly exit_code: number | null;
	readonly error_reason: string | null;
	readonly metadata: SessionMetadata;
	readonly working_directory: string;
}

/**
 * Derived summary returned by session_list. Includes the session fields plus
 * runtime-computed alert, activity_summary, and reattached_mirror flag.
 */
export interface SessionSummary {
	readonly id: number;
	readonly project_id: number;
	readonly label: string;
	readonly pid: number | null;
	readonly status: SessionStatus;
	readonly status_source: "ipc" | "heuristic";
	readonly ownership: SessionOwnership;
	readonly started_at: string;
	readonly ended_at: string | null;
	readonly last_activity_at: string;
	readonly last_heartbeat_at: string | null;
	readonly exit_code: number | null;
	readonly error_reason: string | null;
	readonly metadata: SessionMetadata;
	readonly working_directory: string;
	readonly activity_summary: string | null;
	readonly alert: Alert | null;
	readonly reattached_mirror: boolean;
}

// ---------- Commands ----------

/**
 * List sessions, optionally filtered by project and/or including ended ones.
 */
export async function sessionList(opts?: {
	projectId?: number;
	includeEnded?: boolean;
}): Promise<{ sessions: SessionSummary[] }> {
	return invoke<{ sessions: SessionSummary[] }>("session_list", {
		args: {
			project_id: opts?.projectId,
			include_ended: opts?.includeEnded ?? false,
		},
	});
}

/**
 * Spawn a new workbench-owned session under a PTY.
 */
export async function sessionSpawn(opts: {
	projectId: number;
	command: string[];
	label?: string;
	workingDirectory?: string;
	env?: Record<string, string>;
}): Promise<{ session: Session }> {
	return invoke<{ session: Session }>("session_spawn", {
		args: {
			project_id: opts.projectId,
			label: opts.label,
			command: opts.command,
			working_directory: opts.workingDirectory,
			env: opts.env,
		},
	});
}

/**
 * Fetch the raw-byte replay backlog for a session. Returns the last N bytes
 * (see `REPLAY_CAP` on the backend) emitted by the PTY so a late-attaching
 * pane can restore screen state without waiting for the agent to redraw.
 *
 * Bytes are returned base64-encoded. For wrapper-owned or ended sessions the
 * backlog is empty.
 */
export async function sessionReadBacklog(opts: {
	sessionId: number;
}): Promise<{ bytes: string }> {
	return invoke<{ bytes: string }>("session_read_backlog", {
		args: { session_id: opts.sessionId },
	});
}

/**
 * Send input bytes to a workbench-owned session's PTY stdin.
 */
export async function sessionSendInput(opts: {
	sessionId: number;
	bytes: string;
}): Promise<void> {
	await invoke<Record<string, never>>("session_send_input", {
		args: { session_id: opts.sessionId, bytes: opts.bytes },
	});
}

/**
 * Resize a workbench-owned session's PTY.
 */
export async function sessionResize(opts: {
	sessionId: number;
	cols: number;
	rows: number;
}): Promise<void> {
	await invoke<Record<string, never>>("session_resize", {
		args: { session_id: opts.sessionId, cols: opts.cols, rows: opts.rows },
	});
}

/**
 * End a workbench-owned session by sending a signal to the child process.
 */
export async function sessionEnd(opts: {
	sessionId: number;
	signal?: "TERM" | "KILL";
}): Promise<{ session: Session }> {
	return invoke<{ session: Session }>("session_end", {
		args: { session_id: opts.sessionId, signal: opts.signal },
	});
}

/**
 * Activate a session — ensures a companion terminal exists and returns both.
 */
export async function sessionActivate(opts: { sessionId: number }): Promise<{
	session: Session;
	companion: import("./companions").CompanionTerminal;
}> {
	return invoke<{
		session: Session;
		companion: import("./companions").CompanionTerminal;
	}>("session_activate", { args: { session_id: opts.sessionId } });
}

// ---------- Event subscribers ----------

/**
 * Subscribe to new session spawn events.
 * Returns an unlisten function; call it on component unmount.
 */
export function onSessionSpawned(
	cb: (payload: SessionSpawnedEvent) => void,
): Promise<UnlistenFn> {
	return listen("session:spawned", cb);
}

/**
 * Subscribe to session ended events.
 */
export function onSessionEnded(
	cb: (payload: SessionEndedEvent) => void,
): Promise<UnlistenFn> {
	return listen("session:ended", cb);
}

/**
 * Subscribe to session output (PTY bytes) events.
 */
export function onSessionOutput(
	cb: (payload: SessionOutputEvent) => void,
): Promise<UnlistenFn> {
	return listen("session:event", cb);
}
