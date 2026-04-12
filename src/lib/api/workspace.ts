// T127: typed wrappers for workspace + layout Tauri commands.
// Mirrors contracts/tauri-commands.md §5.

import { invoke } from './invoke';
import { listen } from './events';
import type { UnlistenFn } from '@tauri-apps/api/event';

// ---------- Types ----------

export interface WorkspaceState {
  readonly version: number;
  readonly active_project_ids: number[];
  readonly focused_session_id: number | null;
  readonly pane_layout: string;
  readonly ui: Record<string, unknown>;
}

export interface Layout {
  readonly id: number;
  readonly name: string;
  readonly payload: WorkspaceState;
  readonly created_at: string;
  readonly updated_at: string;
}

// ---------- Workspace commands ----------

export async function workspaceGet(): Promise<{ state: WorkspaceState }> {
  return invoke('workspace_get', {});
}

export async function workspaceSave(state: WorkspaceState): Promise<void> {
  await invoke('workspace_save', { args: { state } });
}

// ---------- Layout commands ----------

export async function layoutList(): Promise<{ layouts: Layout[] }> {
  return invoke('layout_list', {});
}

export async function layoutSave(
  name: string,
  state: WorkspaceState,
  overwrite?: boolean,
): Promise<{ layout: Layout }> {
  return invoke('layout_save', { args: { name, state, overwrite } });
}

export async function layoutRestore(
  id: number,
): Promise<{ state: WorkspaceState; missing_sessions: number[] }> {
  return invoke('layout_restore', { args: { id } });
}

export async function layoutDelete(id: number): Promise<void> {
  await invoke('layout_delete', { args: { id } });
}

// ---------- Events ----------

/** Best-effort startup event listener. The primary hydration path is
 * `workspaceGet()` called from `onMount`. This listener is supplementary. */
export async function onWorkspaceRestored(
  handler: (state: WorkspaceState) => void,
): Promise<UnlistenFn> {
  return listen('workspace:restored', (payload) => {
    handler(payload.state);
  });
}
