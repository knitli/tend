// T128: Svelte 5 runes store for workspace state persistence.
// Debounces (250 ms) saves to the backend. Hydrates before other stores.

import {
  workspaceGet,
  workspaceSave,
  type WorkspaceState,
} from '$lib/api/workspace';

function createWorkspaceStore() {
  let current = $state<WorkspaceState>({
    version: 1,
    active_project_ids: [],
    focused_session_id: null,
    pane_layout: 'split',
    ui: {},
  });
  let loading = $state(false);
  let error = $state<string | null>(null);
  let _saveTimer: ReturnType<typeof setTimeout> | null = null;

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

  /** Set a single UI key. */
  function setUi(key: string, value: unknown): void {
    current = { ...current, ui: { ...current.ui, [key]: value } };
    scheduleSave();
  }

  /** Immediately flush pending saves. Call on unmount / shutdown. */
  async function flush(): Promise<void> {
    if (_saveTimer !== null) {
      clearTimeout(_saveTimer);
      _saveTimer = null;
    }
    try {
      await workspaceSave(current);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function scheduleSave(): void {
    if (_saveTimer !== null) {
      clearTimeout(_saveTimer);
    }
    _saveTimer = setTimeout(async () => {
      _saveTimer = null;
      try {
        await workspaceSave(current);
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
      }
    }, 250);
  }

  return {
    get current() { return current; },
    get loading() { return loading; },
    get error() { return error; },
    hydrate,
    update,
    setUi,
    flush,
  };
}

export const workspaceStore = createWorkspaceStore();
