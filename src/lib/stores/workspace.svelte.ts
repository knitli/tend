// T128: Svelte 5 runes store for workspace state persistence.
// Debounces (250 ms) saves to the backend. Hydrates before other stores.
// M8 fix: inflight guard prevents concurrent saves from racing.

import {
	type WorkspaceState,
	workspaceGet,
	workspaceSave,
} from "$lib/api/workspace";

function createWorkspaceStore() {
	let current = $state<WorkspaceState>({
		version: 1,
		active_project_ids: [],
		focused_session_id: null,
		pane_layout: "split",
		ui: {},
	});
	let loading = $state(false);
	let error = $state<string | null>(null);
	let _saveTimer: ReturnType<typeof setTimeout> | null = null;
	let _inflight: Promise<void> | null = null;

	/** Load workspace state from the backend. Should be called before other
	 * store hydrations on mount. */
	async function hydrate(): Promise<void> {
		loading = true;
		error = null;
		try {
			const { state } = await workspaceGet();
			current = state;
		} catch (err) {
			error = err instanceof Error ? err.message : String(err);
		} finally {
			loading = false;
		}
	}

	/** Update workspace state locally and schedule a debounced save. */
	function update(patch: Partial<WorkspaceState>): void {
		current = { ...current, ...patch };
		scheduleSave();
	}

	/** Full replacement (used by layout restore). */
	function replace(state: WorkspaceState): void {
		current = state;
		scheduleSave();
	}

	/** Set a single UI key. */
	function setUi(key: string, value: unknown): void {
		current = { ...current, ui: { ...current.ui, [key]: value } };
		scheduleSave();
	}

	/** Immediately flush pending saves. Call on unmount / shutdown.
	 * M8 fix: awaits any inflight save before issuing the final write. */
	async function flush(): Promise<void> {
		if (_saveTimer !== null) {
			clearTimeout(_saveTimer);
			_saveTimer = null;
		}
		// Wait for any in-flight save to finish first.
		if (_inflight) {
			await _inflight;
		}
		try {
			_inflight = workspaceSave(current);
			await _inflight;
		} catch (err) {
			error = err instanceof Error ? err.message : String(err);
		} finally {
			_inflight = null;
		}
	}

	function scheduleSave(): void {
		if (_saveTimer !== null) {
			clearTimeout(_saveTimer);
		}
		_saveTimer = setTimeout(async () => {
			_saveTimer = null;
			// M8 fix: chain saves so only one is in-flight at a time.
			if (_inflight) {
				await _inflight;
			}
			try {
				_inflight = workspaceSave(current);
				await _inflight;
			} catch (err) {
				error = err instanceof Error ? err.message : String(err);
			} finally {
				_inflight = null;
			}
		}, 250);
	}

	return {
		get current() {
			return current;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		},
		hydrate,
		update,
		replace,
		setUi,
		flush,
	};
}

export const workspaceStore = createWorkspaceStore();
