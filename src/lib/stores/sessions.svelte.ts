// T062: Svelte 5 runes store for session state.
// Manages a reactive Map of sessions, grouped-by-project derived view,
// and auto-subscribes to backend events for real-time updates.

import type { UnlistenFn } from "@tauri-apps/api/event";
import {
	type AlertClearedEvent,
	type AlertRaisedEvent,
	listen,
} from "$lib/api/events";
import {
	onSessionEnded,
	onSessionOutput,
	onSessionSpawned,
	type SessionStatus,
	type SessionSummary,
	sessionList,
} from "$lib/api/sessions";

export type SessionOutputHandler = (sessionId: number, bytes: string) => void;

function createSessionsStore() {
	let sessionsMap = $state<Map<number, SessionSummary>>(new Map());
	let loading = $state(false);
	let error = $state<string | null>(null);
	let outputHandler = $state<SessionOutputHandler | null>(null);

	const sessions = $derived(Array.from(sessionsMap.values()));

	const activeSessions = $derived(
		sessions.filter((s) => s.status !== "ended" && s.status !== "error"),
	);

	/** Sessions grouped by project_id, sorted by last_activity_at descending. */
	const byProject = $derived.by(() => {
		const groups = new Map<number, SessionSummary[]>();
		for (const session of sessions) {
			const list = groups.get(session.project_id);
			if (list) {
				list.push(session);
			} else {
				groups.set(session.project_id, [session]);
			}
		}
		// Sort each group by last_activity_at descending
		for (const list of groups.values()) {
			list.sort((a, b) => b.last_activity_at.localeCompare(a.last_activity_at));
		}
		return groups;
	});

	/** Sessions that have an active (open) alert. */
	const sessionsWithAlerts = $derived(sessions.filter((s) => s.alert !== null));

	/** Map of open alerts keyed by session id (T081). */
	const alertsBySession = $derived.by(() => {
		const map = new Map<number, SessionSummary["alert"]>();
		for (const s of sessions) {
			if (s.alert) {
				map.set(s.id, s.alert);
			}
		}
		return map;
	});

	/** Total count of open alerts across all sessions (T081). */
	const openAlertCount = $derived(sessionsWithAlerts.length);

	// Event unlisten handles for cleanup
	let unlisteners: UnlistenFn[] = [];

	// C2 fix: Throttled re-hydration to refresh activity_summary.
	// When output events arrive, schedule a re-poll at most every 5 seconds.
	let refreshTimer: ReturnType<typeof setTimeout> | null = null;
	const REFRESH_INTERVAL_MS = 5_000;

	// Per-session throttle for last_activity_at updates. TUIs like Claude
	// emit PTY chunks every ~50 ms (cursor blink, status line). Without
	// throttling, every chunk clones the sessions Map and re-triggers every
	// $derived + component that observes it (SessionList / Sidebar / etc.) —
	// measurable CPU load while a claude session is running. Sub-second
	// activity-timestamp precision is irrelevant for sorting/display.
	const ACTIVITY_THROTTLE_MS = 1_000;
	const lastActivityApplied = new Map<number, number>();

	function scheduleActivityRefresh(store: {
		hydrate: (opts?: { includeEnded?: boolean }) => Promise<void>;
	}): void {
		if (refreshTimer !== null) return; // already scheduled
		refreshTimer = setTimeout(async () => {
			refreshTimer = null;
			try {
				await store.hydrate();
			} catch {
				// Best-effort refresh; errors already captured in store.error.
			}
		}, REFRESH_INTERVAL_MS);
	}

	function cancelActivityRefresh(): void {
		if (refreshTimer !== null) {
			clearTimeout(refreshTimer);
			refreshTimer = null;
		}
	}

	return {
		get sessions() {
			return sessions;
		},
		get activeSessions() {
			return activeSessions;
		},
		get byProject() {
			return byProject;
		},
		get sessionsWithAlerts() {
			return sessionsWithAlerts;
		},
		get alertsBySession() {
			return alertsBySession;
		},
		get openAlertCount() {
			return openAlertCount;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		},

		/** Fetch all sessions from the backend and replace local state. */
		async hydrate(opts?: {
			projectId?: number;
			includeEnded?: boolean;
		}): Promise<void> {
			loading = true;
			error = null;
			try {
				const result = await sessionList(opts);
				const next = new Map<number, SessionSummary>();
				for (const s of result.sessions) {
					next.set(s.id, s);
				}
				sessionsMap = next;
			} catch (err) {
				error = err instanceof Error ? err.message : String(err);
			} finally {
				loading = false;
			}
		},

		/** Add or replace a session in the store. */
		add(session: SessionSummary): void {
			const next = new Map(sessionsMap);
			next.set(session.id, session);
			sessionsMap = next;
		},

		/** Patch fields on an existing session. */
		update(id: number, patch: Partial<SessionSummary>): void {
			const existing = sessionsMap.get(id);
			if (!existing) return;
			const next = new Map(sessionsMap);
			next.set(id, { ...existing, ...patch });
			sessionsMap = next;
		},

		/** Remove a session from the store. */
		remove(id: number): void {
			const next = new Map(sessionsMap);
			next.delete(id);
			sessionsMap = next;
		},

		/** Find a session by id. */
		byId(id: number): SessionSummary | undefined {
			return sessionsMap.get(id);
		},

		/** Get sessions for a specific project. */
		forProject(projectId: number): SessionSummary[] {
			return byProject.get(projectId) ?? [];
		},

		/**
		 * Register a handler for session output bytes.
		 * The handler receives the session id and base64-encoded bytes.
		 */
		setOutputHandler(handler: SessionOutputHandler): void {
			outputHandler = handler;
		},

		/**
		 * Subscribe to backend session events. Call once on app mount.
		 * Returns a cleanup function that unsubscribes all listeners.
		 */
		async subscribe(): Promise<() => void> {
			const spawned = await onSessionSpawned((payload) => {
				// L9 fix: Insert a lightweight placeholder immediately so the UI
				// shows the session, then schedule a hydrate to get the full record
				// from the backend (correct timestamps, working_directory, etc.).
				const lite = payload.session;
				const summary: SessionSummary = {
					id: lite.id,
					project_id: lite.project_id,
					label: lite.label,
					pid: null,
					status: lite.status,
					status_source: "ipc",
					ownership: lite.ownership,
					started_at: new Date().toISOString(),
					ended_at: null,
					last_activity_at: new Date().toISOString(),
					last_heartbeat_at: null,
					exit_code: null,
					error_reason: null,
					metadata: {},
					working_directory: "",
					activity_summary: null,
					alert: null,
					reattached_mirror: lite.reattached_mirror,
				};
				this.add(summary);
				// Re-hydrate to reconcile with actual backend data.
				this.hydrate().catch(() => {});
			});

			const ended = await onSessionEnded((payload) => {
				const status: SessionStatus = "ended";
				this.update(payload.session_id, {
					status,
					ended_at: new Date().toISOString(),
					pid: null,
				});
			});

			const output = await onSessionOutput((payload) => {
				// Forward bytes to the active pane's xterm immediately (this
				// path is hot — TUIs stream chunks continuously).
				if (outputHandler) {
					outputHandler(payload.session_id, payload.bytes);
				}
				// Throttle the activity-timestamp update so we don't re-clone
				// the sessions Map on every chunk. See ACTIVITY_THROTTLE_MS.
				const now = Date.now();
				const last = lastActivityApplied.get(payload.session_id) ?? 0;
				if (now - last >= ACTIVITY_THROTTLE_MS) {
					lastActivityApplied.set(payload.session_id, now);
					this.update(payload.session_id, {
						last_activity_at: new Date(now).toISOString(),
					});
				}
				// C2 fix: Schedule a throttled re-hydration to refresh activity_summary.
				scheduleActivityRefresh(this);
			});

			const alertRaised = await listen(
				"alert:raised",
				(payload: AlertRaisedEvent) => {
					const alert = payload.alert;
					this.update(alert.session_id, { alert });
				},
			);

			const alertCleared = await listen(
				"alert:cleared",
				(payload: AlertClearedEvent) => {
					// Find the session that owns this alert and clear it.
					// Read from sessionsMap directly (not the derived `sessions` array)
					// to avoid a stale closure over the derived snapshot.
					for (const [id, session] of sessionsMap) {
						if (session.alert?.id === payload.alert_id) {
							this.update(id, { alert: null });
							break;
						}
					}
				},
			);

			unlisteners = [spawned, ended, output, alertRaised, alertCleared];

			return () => {
				for (const unlisten of unlisteners) {
					unlisten();
				}
				unlisteners = [];
				cancelActivityRefresh();
			};
		},
	};
}

export const sessionsStore = createSessionsStore();
