// T114: Per-project scratchpad store (notes + reminders).
// Lazy-loaded when a project is opened.

import {
  noteList,
  noteCreate,
  noteUpdate,
  noteDelete,
  reminderList,
  reminderCreate,
  reminderSetState,
  reminderDelete,
  type Note,
  type Reminder,
  type ReminderState,
} from '$lib/api/scratchpad';
import { overviewStore } from '$lib/stores/overview.svelte';

class ScratchpadStore {
  notes = $state<Note[]>([]);
  reminders = $state<Reminder[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  projectId = $state<number | null>(null);

  // H5 fix: track the in-flight request to detect stale responses.
  private _loadSeq = 0;

  /** Load notes and reminders for a project.
   *  H4 fix: accepts `force` param to bypass cache.
   *  H5 fix: discards stale responses on rapid project switch. */
  async load(projectId: number, force = false) {
    if (!force && this.projectId === projectId) return;
    this.projectId = projectId;
    this.loading = true;
    this.error = null;

    const seq = ++this._loadSeq;

    try {
      const [notesResult, remindersResult] = await Promise.all([
        noteList({ projectId }),
        reminderList({ projectId }),
      ]);
      // H5 fix: discard if a newer load was started while we were awaiting.
      if (this._loadSeq !== seq) return;
      this.notes = notesResult.notes;
      this.reminders = remindersResult.reminders;
    } catch (err: unknown) {
      if (this._loadSeq !== seq) return;
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      if (this._loadSeq === seq) {
        this.loading = false;
      }
    }
  }

  /** Add a new note. */
  async addNote(content: string) {
    if (!this.projectId) return;
    try {
      const result = await noteCreate({ projectId: this.projectId, content });
      this.notes = [result.note, ...this.notes];
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /** Update a note's content. */
  async updateNote(id: number, content: string) {
    try {
      const result = await noteUpdate({ id, content });
      this.notes = this.notes.map((n) => (n.id === id ? result.note : n));
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /** Delete a note. */
  async removeNote(id: number) {
    try {
      await noteDelete({ id });
      this.notes = this.notes.filter((n) => n.id !== id);
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /** Add a new reminder. */
  async addReminder(content: string) {
    if (!this.projectId) return;
    try {
      const result = await reminderCreate({ projectId: this.projectId, content });
      this.reminders = [result.reminder, ...this.reminders];
      // H7 fix: refresh overview when reminders change.
      overviewStore.refresh();
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /** Toggle a reminder's state. */
  async toggleReminder(id: number, state: ReminderState) {
    try {
      const result = await reminderSetState({ id, state });
      this.reminders = this.reminders.map((r) => (r.id === id ? result.reminder : r));
      // H7 fix: refresh overview when reminder state changes.
      overviewStore.refresh();
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /** Delete a reminder. */
  async removeReminder(id: number) {
    try {
      await reminderDelete({ id });
      this.reminders = this.reminders.filter((r) => r.id !== id);
      // H7 fix: refresh overview when reminders change.
      overviewStore.refresh();
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  /** Clear the store when switching projects. */
  clear() {
    this.projectId = null;
    this.notes = [];
    this.reminders = [];
    this.error = null;
  }
}

export const scratchpadStore = new ScratchpadStore();
