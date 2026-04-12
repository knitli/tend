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

class ScratchpadStore {
  notes = $state<Note[]>([]);
  reminders = $state<Reminder[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  projectId = $state<number | null>(null);

  /** Load notes and reminders for a project. */
  async load(projectId: number) {
    if (this.projectId === projectId) return;
    this.projectId = projectId;
    this.loading = true;
    this.error = null;

    try {
      const [notesResult, remindersResult] = await Promise.all([
        noteList({ projectId }),
        reminderList({ projectId }),
      ]);
      this.notes = notesResult.notes;
      this.reminders = remindersResult.reminders;
    } catch (err: unknown) {
      this.error = err instanceof Error ? err.message : String(err);
    } finally {
      this.loading = false;
    }
  }

  /** Add a new note. */
  async addNote(content: string) {
    if (!this.projectId) return;
    const result = await noteCreate({ projectId: this.projectId, content });
    this.notes = [result.note, ...this.notes];
  }

  /** Update a note's content. */
  async updateNote(id: number, content: string) {
    const result = await noteUpdate({ id, content });
    this.notes = this.notes.map((n) => (n.id === id ? result.note : n));
  }

  /** Delete a note. */
  async removeNote(id: number) {
    await noteDelete({ id });
    this.notes = this.notes.filter((n) => n.id !== id);
  }

  /** Add a new reminder. */
  async addReminder(content: string) {
    if (!this.projectId) return;
    const result = await reminderCreate({ projectId: this.projectId, content });
    this.reminders = [result.reminder, ...this.reminders];
  }

  /** Toggle a reminder's state. */
  async toggleReminder(id: number, state: ReminderState) {
    const result = await reminderSetState({ id, state });
    this.reminders = this.reminders.map((r) => (r.id === id ? result.reminder : r));
  }

  /** Delete a reminder. */
  async removeReminder(id: number) {
    await reminderDelete({ id });
    this.reminders = this.reminders.filter((r) => r.id !== id);
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
