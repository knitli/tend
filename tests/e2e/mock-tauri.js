/**
 * In-memory Tauri IPC mock injected via Playwright's `addInitScript` before
 * the page loads. Stubs `window.__TAURI_INTERNALS__` so that
 * `@tauri-apps/api/core::invoke` and `@tauri-apps/api/event::listen` work
 * inside the browser without a real Tauri backend.
 *
 * Backed by an in-memory store; reset per spec via `window.__MOCK_TAURI__.reset()`
 * (the fixture wires this into beforeEach).
 *
 * IMPORTANT: This file is loaded as raw text and executed in the browser context.
 * Do not add ESM imports or Node globals.
 */

(() => {
	// sessionStorage scope: Playwright gives every test its own browser context
	// (and therefore its own sessionStorage), so persisting here gives us
	// "survives reload" semantics within a test without leaking between tests.
	const STORAGE_KEY = "__mock_tauri_state__";

	// Maps and Sets do not JSON-serialize natively; we persist *only* the
	// data fields (Maps as entry arrays, plain arrays as-is) and leave
	// event-loop concerns (callbacks, eventHandlers) out — those are owned by
	// the live JS module and would be invalid after reload anyway.
	const MAP_KEYS = [
		"projects",
		"sessions",
		"companions",
		"alerts",
		"layouts",
		"notificationPrefs",
	];
	const ARRAY_KEYS = ["notes", "reminders"];
	const SCALAR_KEYS = [
		"nextProjectId",
		"nextSessionId",
		"nextCompanionId",
		"nextNoteId",
		"nextReminderId",
		"nextLayoutId",
		"nextAlertId",
		"nextEventId",
		"nextCallbackId",
		"workspace",
	];

	function makeState() {
		return {
			nextProjectId: 1,
			nextSessionId: 1,
			nextCompanionId: 1,
			nextNoteId: 1,
			nextReminderId: 1,
			nextLayoutId: 1,
			nextAlertId: 1,
			nextEventId: 1,
			nextCallbackId: 1,
			projects: new Map(),
			sessions: new Map(),
			companions: new Map(),
			alerts: new Map(),
			notes: [],
			reminders: [],
			layouts: new Map(),
			workspace: null,
			notificationPrefs: new Map(),
			eventHandlers: new Map(),
			callbacks: new Map(),
		};
	}

	function loadState() {
		const fresh = makeState();
		let raw;
		try {
			raw = window.sessionStorage.getItem(STORAGE_KEY);
		} catch {
			return fresh;
		}
		if (!raw) return fresh;
		let snap;
		try {
			snap = JSON.parse(raw);
		} catch {
			return fresh;
		}
		for (const k of SCALAR_KEYS) {
			if (k in snap) fresh[k] = snap[k];
		}
		for (const k of ARRAY_KEYS) {
			if (Array.isArray(snap[k])) fresh[k] = snap[k];
		}
		for (const k of MAP_KEYS) {
			if (Array.isArray(snap[k])) fresh[k] = new Map(snap[k]);
		}
		return fresh;
	}

	function persistState() {
		try {
			const snap = {};
			for (const k of SCALAR_KEYS) snap[k] = state[k];
			for (const k of ARRAY_KEYS) snap[k] = state[k];
			for (const k of MAP_KEYS) snap[k] = Array.from(state[k].entries());
			window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(snap));
		} catch {
			// sessionStorage may be unavailable in some contexts; safe to ignore —
			// the tests that need cross-reload state will surface the failure.
		}
	}

	let state = loadState();

	function nowIso() {
		return new Date().toISOString();
	}

	function transformCallback(cb, once) {
		const id = state.nextCallbackId++;
		state.callbacks.set(id, { fn: cb, once: !!once });
		return id;
	}

	function unregisterCallback(id) {
		state.callbacks.delete(id);
	}

	function emitEvent(eventName, payload) {
		const handlers = state.eventHandlers.get(eventName);
		if (!handlers) return;
		for (const [eventId, callbackId] of Array.from(handlers.entries())) {
			const cb = state.callbacks.get(callbackId);
			if (!cb) continue;
			try {
				cb.fn({ event: eventName, id: eventId, payload });
			} catch (e) {
				// eslint-disable-next-line no-console
				console.error("[mock-tauri] event handler threw", e);
			}
			if (cb.once) {
				state.callbacks.delete(callbackId);
				handlers.delete(eventId);
			}
		}
	}

	function deferEmit(eventName, payload) {
		setTimeout(() => emitEvent(eventName, payload), 0);
	}

	function makeProject(id, path, displayName) {
		const name = displayName || (path.split("/").filter(Boolean).pop() ?? path);
		return {
			id,
			canonical_path: path,
			display_name: name,
			added_at: nowIso(),
			last_active_at: null,
			archived_at: null,
			settings: { color: "#60a5fa" },
		};
	}

	function makeSession(id, projectId, label, command, workingDirectory) {
		return {
			id,
			project_id: projectId,
			label: label ?? `session-${id}`,
			pid: 12345 + id,
			status: "idle",
			status_source: "heuristic",
			ownership: "workbench",
			started_at: nowIso(),
			ended_at: null,
			last_activity_at: nowIso(),
			last_heartbeat_at: null,
			exit_code: null,
			error_reason: null,
			metadata: { command: command ?? null },
			working_directory: workingDirectory || "/tmp",
		};
	}

	function summaryFor(session) {
		const alert =
			Array.from(state.alerts.values()).find(
				(a) => a.session_id === session.id,
			) ?? null;
		return {
			...session,
			activity_summary: null,
			alert,
			reattached_mirror: false,
		};
	}

	function err(code, message, details) {
		const e = { code, message };
		if (details) e.details = details;
		return e;
	}

	const handlers = {
		// ---------- Projects ----------
		project_register({ path, display_name }) {
			const id = state.nextProjectId++;
			const p = makeProject(id, path, display_name);
			state.projects.set(id, p);
			return { project: p };
		},
		project_list({ include_archived } = {}) {
			const projects = Array.from(state.projects.values()).filter(
				(p) => include_archived || !p.archived_at,
			);
			return { projects };
		},
		project_update({ id, display_name, settings }) {
			const p = state.projects.get(id);
			if (!p) throw err("NOT_FOUND", "Project not found");
			const updated = {
				...p,
				display_name: display_name ?? p.display_name,
				settings: { ...p.settings, ...(settings ?? {}) },
			};
			state.projects.set(id, updated);
			return { project: updated };
		},
		project_archive({ id }) {
			const p = state.projects.get(id);
			if (p) state.projects.set(id, { ...p, archived_at: nowIso() });
			return {};
		},
		project_unarchive({ id }) {
			const p = state.projects.get(id);
			if (!p || !p.archived_at)
				throw err("NOT_ARCHIVED", "Project is not archived");
			const updated = { ...p, archived_at: null };
			state.projects.set(id, updated);
			return { project: updated };
		},

		// ---------- Sessions ----------
		session_spawn({ project_id, label, command, working_directory }) {
			const id = state.nextSessionId++;
			const s = makeSession(id, project_id, label, command, working_directory);
			state.sessions.set(id, s);
			deferEmit("session:spawned", { session: summaryFor(s) });
			return { session: s };
		},
		session_list({ project_id, include_ended } = {}) {
			let arr = Array.from(state.sessions.values());
			if (project_id != null)
				arr = arr.filter((s) => s.project_id === project_id);
			if (!include_ended) arr = arr.filter((s) => s.status !== "ended");
			return { sessions: arr.map(summaryFor) };
		},
		session_end({ session_id }) {
			const s = state.sessions.get(session_id);
			if (!s) throw err("NOT_FOUND", "Session not found");
			const ended = {
				...s,
				status: "ended",
				ended_at: nowIso(),
				exit_code: 0,
			};
			state.sessions.set(session_id, ended);
			deferEmit("session:ended", { session_id, code: 0 });
			return { session: ended };
		},
		session_activate({ session_id }) {
			const s = state.sessions.get(session_id);
			if (!s) throw err("NOT_FOUND", "Session not found");
			let c = state.companions.get(session_id);
			if (!c) {
				c = {
					id: state.nextCompanionId++,
					session_id,
					pid: 54321 + session_id,
					shell_path: "/bin/sh",
					initial_cwd: s.working_directory,
					started_at: nowIso(),
					ended_at: null,
				};
				state.companions.set(session_id, c);
				deferEmit("companion:spawned", {
					session_id,
					companion: { id: c.id, pid: c.pid },
				});
			}
			return { session: s, companion: c };
		},
		session_set_focus() {
			return {};
		},
		session_set_visible() {
			return {};
		},
		session_send_input() {
			return {};
		},
		session_resize() {
			return {};
		},
		session_read_backlog() {
			return { bytes: "" };
		},
		session_acknowledge_alert({ alert_id }) {
			const a = state.alerts.get(alert_id);
			state.alerts.delete(alert_id);
			if (a) deferEmit("alert:cleared", { alert_id, by: "user" });
			return {};
		},

		// ---------- Companions ----------
		companion_respawn({ session_id }) {
			const s = state.sessions.get(session_id);
			if (!s) throw err("NOT_FOUND", "Session not found");
			const c = {
				id: state.nextCompanionId++,
				session_id,
				pid: 99999 + session_id,
				shell_path: "/bin/sh",
				initial_cwd: s.working_directory,
				started_at: nowIso(),
				ended_at: null,
			};
			state.companions.set(session_id, c);
			deferEmit("companion:spawned", {
				session_id,
				companion: { id: c.id, pid: c.pid },
			});
			return { companion: c };
		},
		companion_send_input() {
			return {};
		},
		companion_resize() {
			return {};
		},

		// ---------- Notes & Reminders ----------
		note_create({ project_id, content }) {
			const note = {
				id: state.nextNoteId++,
				project_id,
				content,
				created_at: nowIso(),
				updated_at: nowIso(),
			};
			state.notes.push(note);
			return { note };
		},
		note_list({ project_id } = {}) {
			return {
				notes: state.notes.filter(
					(n) => project_id == null || n.project_id === project_id,
				),
			};
		},
		note_update({ id, content }) {
			const i = state.notes.findIndex((n) => n.id === id);
			if (i < 0) throw err("NOT_FOUND", "Note not found");
			state.notes[i] = {
				...state.notes[i],
				content,
				updated_at: nowIso(),
			};
			return { note: state.notes[i] };
		},
		note_delete({ id }) {
			state.notes = state.notes.filter((n) => n.id !== id);
			return {};
		},

		reminder_create({ project_id, content }) {
			const reminder = {
				id: state.nextReminderId++,
				project_id,
				content,
				state: "open",
				created_at: nowIso(),
				completed_at: null,
			};
			state.reminders.push(reminder);
			return { reminder };
		},
		reminder_list({ project_id, states } = {}) {
			let arr = state.reminders;
			if (project_id != null)
				arr = arr.filter((r) => r.project_id === project_id);
			if (states && states.length)
				arr = arr.filter((r) => states.includes(r.state));
			return { reminders: arr };
		},
		reminder_set_state({ id, state: s }) {
			const i = state.reminders.findIndex((r) => r.id === id);
			if (i < 0) throw err("NOT_FOUND", "Reminder not found");
			state.reminders[i] = {
				...state.reminders[i],
				state: s,
				completed_at: s === "done" ? nowIso() : null,
			};
			return { reminder: state.reminders[i] };
		},
		reminder_delete({ id }) {
			state.reminders = state.reminders.filter((r) => r.id !== id);
			return {};
		},

		cross_project_overview() {
			const groups = [];
			for (const p of state.projects.values()) {
				const reminders = state.reminders.filter(
					(r) => r.project_id === p.id && r.state === "open",
				);
				if (reminders.length === 0) continue;
				groups.push({
					project_id: p.id,
					project_display_name: p.display_name,
					reminders,
				});
			}
			return { groups };
		},

		// ---------- Workspace & layouts ----------
		workspace_get() {
			return {
				state: state.workspace ?? {
					version: 1,
					active_project_ids: [],
					focused_session_id: null,
					pane_layout: "split",
					ui: {},
				},
			};
		},
		workspace_save({ state: s }) {
			state.workspace = s;
			return {};
		},

		layout_save({ name, state: s, overwrite }) {
			const existing = Array.from(state.layouts.values()).find(
				(l) => l.name === name,
			);
			if (existing && !overwrite)
				throw err("NAME_TAKEN", `Layout name ${name} is taken`);
			if (existing) {
				const updated = { ...existing, state: s, updated_at: nowIso() };
				state.layouts.set(existing.id, updated);
				return { layout: updated };
			}
			const layout = {
				id: state.nextLayoutId++,
				name,
				state: s,
				created_at: nowIso(),
				updated_at: nowIso(),
			};
			state.layouts.set(layout.id, layout);
			return { layout };
		},
		layout_list() {
			return { layouts: Array.from(state.layouts.values()) };
		},
		layout_restore({ id }) {
			const l = state.layouts.get(id);
			if (!l) throw err("NOT_FOUND", "Layout not found");
			return { state: l.state, missing_sessions: [] };
		},
		layout_delete({ id }) {
			state.layouts.delete(id);
			return {};
		},

		// ---------- Notifications ----------
		notification_preference_get({ project_id }) {
			const pref = state.notificationPrefs.get(project_id) ?? {
				project_id,
				quiet_hours_start: null,
				quiet_hours_end: null,
				muted: false,
			};
			return { preference: pref };
		},
		notification_preference_set({
			project_id,
			quiet_hours_start,
			quiet_hours_end,
			muted,
		}) {
			const pref = {
				project_id,
				quiet_hours_start: quiet_hours_start ?? null,
				quiet_hours_end: quiet_hours_end ?? null,
				muted: muted ?? false,
			};
			state.notificationPrefs.set(project_id, pref);
			return { preference: pref };
		},

		// ---------- Tauri event plugin ----------
		"plugin:event|listen"({ event, handler }) {
			const eventId = state.nextEventId++;
			let handlers = state.eventHandlers.get(event);
			if (!handlers) {
				handlers = new Map();
				state.eventHandlers.set(event, handlers);
			}
			handlers.set(eventId, handler);
			return eventId;
		},
		"plugin:event|unlisten"({ event, eventId }) {
			const handlers = state.eventHandlers.get(event);
			if (handlers) handlers.delete(eventId);
			return null;
		},
		"plugin:event|emit"({ event, payload }) {
			emitEvent(event, payload);
			return null;
		},
		"plugin:event|emit_to"({ event, payload }) {
			emitEvent(event, payload);
			return null;
		},
	};

	function unwrapArgs(raw) {
		if (raw && typeof raw === "object" && "args" in raw) return raw.args ?? {};
		return raw ?? {};
	}

	// Commands that mutate persistent state — only these trigger sessionStorage
	// writes. Skipping read-only commands keeps the storage write rate sane.
	const MUTATING = new Set([
		"project_register",
		"project_update",
		"project_archive",
		"project_unarchive",
		"session_spawn",
		"session_end",
		"session_activate",
		"session_acknowledge_alert",
		"companion_respawn",
		"note_create",
		"note_update",
		"note_delete",
		"reminder_create",
		"reminder_set_state",
		"reminder_delete",
		"workspace_save",
		"layout_save",
		"layout_delete",
		"notification_preference_set",
	]);

	function invoke(cmd, args /* , options */) {
		const handler = handlers[cmd];
		if (!handler) {
			// eslint-disable-next-line no-console
			console.warn("[mock-tauri] unhandled command:", cmd, args);
			return Promise.reject(err("INTERNAL", `mock: unhandled command ${cmd}`));
		}
		try {
			const flat = unwrapArgs(args);
			const result = handler(flat);
			if (MUTATING.has(cmd)) persistState();
			return Promise.resolve(result);
		} catch (e) {
			return Promise.reject(e);
		}
	}

	window.__TAURI_INTERNALS__ = {
		invoke,
		transformCallback,
		unregisterCallback,
		metadata: {
			currentWindow: { label: "main" },
			currentWebview: { label: "main", windowLabel: "main" },
		},
	};

	// Test-side control surface — used by helpers and specs to reset state and
	// simulate backend-side state changes (e.g. raising an alert that a real
	// daemon IPC path would normally trigger).
	window.__MOCK_TAURI__ = {
		get state() {
			return state;
		},
		emitEvent,
		raiseAlert(sessionId, reason) {
			const session = state.sessions.get(sessionId);
			if (!session) throw new Error(`mock: no such session ${sessionId}`);
			const alert = {
				id: state.nextAlertId++,
				session_id: sessionId,
				project_id: session.project_id,
				reason: reason ?? "needs_input",
				raised_at: nowIso(),
			};
			state.alerts.set(alert.id, alert);
			state.sessions.set(sessionId, { ...session, status: "needs_input" });
			persistState();
			deferEmit("alert:raised", { alert });
			return alert;
		},
		reset() {
			state = makeState();
			try {
				window.sessionStorage.removeItem(STORAGE_KEY);
			} catch {
				/* ignore */
			}
		},
	};
})();
