// T095: typed wrappers for companion terminal Tauri commands and event subscribers.
// Mirrors the companion surface from contracts/tauri-commands.md §3.

import type { UnlistenFn } from "@tauri-apps/api/event";
import {
	type CompanionOutputEvent,
	type CompanionSpawnedEvent,
	listen,
} from "./events";
import { invoke } from "./invoke";

// ---------- Types ----------

export interface CompanionTerminal {
	readonly id: number;
	readonly session_id: number;
	readonly pid: number | null;
	readonly shell_path: string;
	readonly initial_cwd: string;
	readonly started_at: string;
	readonly ended_at: string | null;
}

// ---------- Commands ----------

/**
 * Send input bytes to a companion terminal's PTY stdin.
 */
export async function companionSendInput(opts: {
	sessionId: number;
	bytes: string;
}): Promise<void> {
	await invoke<Record<string, never>>("companion_send_input", {
		args: { session_id: opts.sessionId, bytes: opts.bytes },
	});
}

/**
 * Resize a companion terminal's PTY.
 */
export async function companionResize(opts: {
	sessionId: number;
	cols: number;
	rows: number;
}): Promise<void> {
	await invoke<Record<string, never>>("companion_resize", {
		args: { session_id: opts.sessionId, cols: opts.cols, rows: opts.rows },
	});
}

/**
 * Forcibly recreate a companion terminal.
 */
export async function companionRespawn(opts: {
	sessionId: number;
}): Promise<{ companion: CompanionTerminal }> {
	return invoke<{ companion: CompanionTerminal }>("companion_respawn", {
		args: { session_id: opts.sessionId },
	});
}

// ---------- Event subscribers ----------

/**
 * Subscribe to companion spawned events.
 */
export function onCompanionSpawned(
	cb: (payload: CompanionSpawnedEvent) => void,
): Promise<UnlistenFn> {
	return listen("companion:spawned", cb);
}

/**
 * Subscribe to companion output (PTY bytes) events.
 */
export function onCompanionOutput(
	cb: (payload: CompanionOutputEvent) => void,
): Promise<UnlistenFn> {
	return listen("companion:output", cb);
}
