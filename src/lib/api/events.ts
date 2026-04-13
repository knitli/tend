// T029: typed wrapper around `@tauri-apps/api/event::listen` with a compile-
// time-known event map. Store modules subscribe through these helpers so the
// wire format stays single-source from the backend.

import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------- Event payload shapes ----------
// These are pinned by contract tests on the backend side; the frontend re-
// declares the TypeScript-level shape for type safety in the UI layer.

/** Minimal session summary shape. Full fields arrive from `session_list`. */
export interface SessionSummaryLite {
	readonly id: number;
	readonly project_id: number;
	readonly label: string;
	readonly status: "working" | "idle" | "needs_input" | "ended" | "error";
	readonly ownership: "workbench" | "wrapper";
	readonly reattached_mirror: boolean;
}

export interface SessionSpawnedEvent {
	readonly session: SessionSummaryLite;
}

export interface SessionEndedEvent {
	readonly session_id: number;
	readonly code?: number;
}

export interface SessionOutputEvent {
	readonly session_id: number;
	/** Base64-encoded bytes. */
	readonly bytes: string;
}

export interface AlertRaisedEvent {
	readonly alert: {
		readonly id: number;
		readonly session_id: number;
		readonly project_id: number;
		readonly reason?: string;
		readonly raised_at: string;
	};
}

export interface AlertClearedEvent {
	readonly alert_id: number;
	readonly by: "user" | "session_resumed" | "session_ended";
}

export interface ProjectPathMissingEvent {
	readonly project_id: number;
}

export interface ProjectPathRestoredEvent {
	readonly project_id: number;
}

export interface CompanionSpawnedEvent {
	readonly session_id: number;
	readonly companion: { readonly id: number; readonly pid: number | null };
}

export interface CompanionOutputEvent {
	readonly session_id: number;
	readonly bytes: string;
}

/** Best-effort startup event — may be missed if the frontend listener
 * attaches after the backend emits. Always call `workspace_get` on mount
 * as the primary hydration path (M1 documented). */
export interface WorkspaceRestoredEvent {
	readonly state: {
		readonly version: number;
		readonly active_project_ids: number[];
		readonly focused_session_id: number | null;
		readonly pane_layout: string;
		readonly ui: Record<string, unknown>;
	};
}

/** Map from event name → payload. Used by [`listen`] for type safety. */
export interface WorkbenchEventMap {
	"session:spawned": SessionSpawnedEvent;
	"session:ended": SessionEndedEvent;
	"session:event": SessionOutputEvent;
	"alert:raised": AlertRaisedEvent;
	"alert:cleared": AlertClearedEvent;
	"project:path_missing": ProjectPathMissingEvent;
	"project:path_restored": ProjectPathRestoredEvent;
	"companion:spawned": CompanionSpawnedEvent;
	"companion:output": CompanionOutputEvent;
	"workspace:restored": WorkspaceRestoredEvent;
}

/**
 * Subscribe to a workbench event.
 *
 * Returns an unlisten function; call it on component unmount.
 */
export async function listen<K extends keyof WorkbenchEventMap>(
	event: K,
	handler: (payload: WorkbenchEventMap[K]) => void,
): Promise<UnlistenFn> {
	return tauriListen<WorkbenchEventMap[K]>(event, (e) => handler(e.payload));
}
