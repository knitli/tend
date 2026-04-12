// T111: typed wrappers for scratchpad Tauri commands.
// Mirrors contracts/tauri-commands.md §4.

import { invoke } from './invoke';

// ---------- Types ----------

export interface Note {
  readonly id: number;
  readonly project_id: number;
  readonly content: string;
  readonly created_at: string;
  readonly updated_at: string;
}

export type ReminderState = 'open' | 'done';

export interface Reminder {
  readonly id: number;
  readonly project_id: number;
  readonly content: string;
  readonly state: ReminderState;
  readonly created_at: string;
  readonly done_at: string | null;
}

export interface OverviewGroup {
  readonly project: {
    readonly id: number;
    readonly display_name: string;
    readonly canonical_path: string;
  };
  readonly open_reminders: Reminder[];
}

// ---------- Note commands ----------

export async function noteList(
  opts: { projectId: number; limit?: number; cursor?: string },
): Promise<{ notes: Note[]; next_cursor?: string }> {
  return invoke('note_list', {
    args: { project_id: opts.projectId, limit: opts.limit, cursor: opts.cursor },
  });
}

export async function noteCreate(
  opts: { projectId: number; content: string },
): Promise<{ note: Note }> {
  return invoke('note_create', {
    args: { project_id: opts.projectId, content: opts.content },
  });
}

export async function noteUpdate(
  opts: { id: number; content: string },
): Promise<{ note: Note }> {
  return invoke('note_update', {
    args: { id: opts.id, content: opts.content },
  });
}

export async function noteDelete(opts: { id: number }): Promise<void> {
  await invoke('note_delete', { args: { id: opts.id } });
}

// ---------- Reminder commands ----------

export async function reminderList(
  opts?: { projectId?: number; state?: ReminderState; limit?: number; cursor?: string },
): Promise<{ reminders: Reminder[]; next_cursor?: string }> {
  return invoke('reminder_list', {
    args: {
      project_id: opts?.projectId,
      state: opts?.state,
      limit: opts?.limit,
      cursor: opts?.cursor,
    },
  });
}

export async function reminderCreate(
  opts: { projectId: number; content: string },
): Promise<{ reminder: Reminder }> {
  return invoke('reminder_create', {
    args: { project_id: opts.projectId, content: opts.content },
  });
}

export async function reminderSetState(
  opts: { id: number; state: ReminderState },
): Promise<{ reminder: Reminder }> {
  return invoke('reminder_set_state', {
    args: { id: opts.id, state: opts.state },
  });
}

export async function reminderDelete(opts: { id: number }): Promise<void> {
  await invoke('reminder_delete', { args: { id: opts.id } });
}

// ---------- Overview ----------

export async function crossProjectOverview(): Promise<{ groups: OverviewGroup[] }> {
  return invoke('cross_project_overview', {});
}
