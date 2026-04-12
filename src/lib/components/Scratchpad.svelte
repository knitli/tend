<!--
  T115: Per-project scratchpad with Notes and Reminders tabs.
-->
<script lang="ts">
  import { scratchpadStore } from '$lib/stores/scratchpad.svelte';
  import { formatAge } from '$lib/util/age';
  import { renderInlineMarkdown } from '$lib/util/markdown';
  import type { ReminderState } from '$lib/api/scratchpad';

  interface Props {
    projectId: number;
  }

  let { projectId }: Props = $props();

  let activeTab = $state<'notes' | 'reminders'>('notes');
  let newNoteContent = $state('');
  let newReminderContent = $state('');

  // H6 fix: single $effect handles both initial load and project changes.
  $effect(() => {
    scratchpadStore.load(projectId);
  });

  async function handleAddNote() {
    const content = newNoteContent.trim();
    if (!content) return;
    try {
      await scratchpadStore.addNote(content);
      newNoteContent = '';
    } catch {
      // Error is in the store.
    }
  }

  async function handleDeleteNote(id: number) {
    await scratchpadStore.removeNote(id);
  }

  async function handleAddReminder() {
    const content = newReminderContent.trim();
    if (!content) return;
    try {
      await scratchpadStore.addReminder(content);
      newReminderContent = '';
    } catch {
      // Error is in the store.
    }
  }

  async function handleToggleReminder(id: number, currentState: ReminderState) {
    const newState: ReminderState = currentState === 'open' ? 'done' : 'open';
    await scratchpadStore.toggleReminder(id, newState);
  }

  async function handleDeleteReminder(id: number) {
    await scratchpadStore.removeReminder(id);
  }

  function handleNoteKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleAddNote();
    }
  }

  function handleReminderKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAddReminder();
    }
  }
</script>

<div class="scratchpad" role="region" aria-label="Project scratchpad">
  <div class="tab-bar" role="tablist">
    <button
      id="tab-notes"
      class="tab"
      class:active={activeTab === 'notes'}
      role="tab"
      aria-selected={activeTab === 'notes'}
      aria-controls="panel-notes"
      onclick={() => activeTab = 'notes'}
    >
      Notes ({scratchpadStore.notes.length})
    </button>
    <button
      id="tab-reminders"
      class="tab"
      class:active={activeTab === 'reminders'}
      role="tab"
      aria-selected={activeTab === 'reminders'}
      aria-controls="panel-reminders"
      onclick={() => activeTab = 'reminders'}
    >
      Reminders ({scratchpadStore.reminders.length})
    </button>
  </div>

  {#if scratchpadStore.loading}
    <p class="empty">Loading...</p>
  {:else if activeTab === 'notes'}
    <div class="tab-content" id="panel-notes" role="tabpanel" aria-labelledby="tab-notes">
      <div class="input-row">
        <textarea
          class="note-input"
          bind:value={newNoteContent}
          placeholder="Add a note... (Ctrl+Enter to save)"
          aria-label="New note content"
          rows="2"
          onkeydown={handleNoteKeydown}
        ></textarea>
        <button class="add-btn" onclick={handleAddNote} disabled={!newNoteContent.trim()}>
          Add
        </button>
      </div>

      {#if scratchpadStore.notes.length === 0}
        <p class="empty">No notes yet.</p>
      {:else}
        <ul class="item-list">
          {#each scratchpadStore.notes as note (note.id)}
            <li class="note-item">
              <div class="note-content">{@html renderInlineMarkdown(note.content)}</div>
              <div class="item-meta">
                <span class="age">{formatAge(note.created_at)}</span>
                <button class="delete-btn" onclick={() => handleDeleteNote(note.id)} title="Delete note">
                  Delete
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {:else}
    <div class="tab-content" id="panel-reminders" role="tabpanel" aria-labelledby="tab-reminders">
      <div class="input-row">
        <input
          type="text"
          class="reminder-input"
          bind:value={newReminderContent}
          placeholder="Add a reminder... (Enter to save)"
          aria-label="New reminder content"
          onkeydown={handleReminderKeydown}
        />
        <button class="add-btn" onclick={handleAddReminder} disabled={!newReminderContent.trim()}>
          Add
        </button>
      </div>

      {#if scratchpadStore.reminders.length === 0}
        <p class="empty">No reminders yet.</p>
      {:else}
        <ul class="item-list">
          {#each scratchpadStore.reminders as reminder (reminder.id)}
            <li class="reminder-item" class:done={reminder.state === 'done'}>
              <label class="reminder-label">
                <input
                  type="checkbox"
                  checked={reminder.state === 'done'}
                  onchange={() => handleToggleReminder(reminder.id, reminder.state)}
                />
                <span class="reminder-content">{reminder.content}</span>
              </label>
              <div class="item-meta">
                <span class="age">{formatAge(reminder.created_at)}</span>
                <button class="delete-btn" onclick={() => handleDeleteReminder(reminder.id)} title="Delete reminder">
                  Delete
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  .scratchpad {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
  }

  .tab-bar {
    display: flex;
    border-bottom: 1px solid var(--color-border, #2a2d35);
    flex-shrink: 0;
  }

  .tab {
    flex: 1;
    padding: 0.5rem 0.75rem;
    border: none;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    transition: color 150ms, border-color 150ms;
    border-bottom: 2px solid transparent;
  }

  .tab.active {
    color: var(--color-text, #e6e8ef);
    border-bottom-color: var(--color-accent, #60a5fa);
  }

  .tab:hover:not(.active) {
    color: var(--color-text, #e6e8ef);
  }

  .tab-content {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
  }

  .input-row {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .note-input,
  .reminder-input {
    flex: 1;
    padding: 0.375rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 4px;
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
    resize: vertical;
  }

  .note-input:focus,
  .reminder-input:focus {
    outline: none;
    border-color: var(--color-accent, #60a5fa);
  }

  .add-btn {
    padding: 0.375rem 0.75rem;
    border: 1px solid var(--color-accent, #60a5fa);
    border-radius: 4px;
    background: transparent;
    color: var(--color-accent, #60a5fa);
    cursor: pointer;
    font-size: 0.75rem;
    font-weight: 600;
    white-space: nowrap;
    align-self: flex-end;
  }

  .add-btn:hover:not(:disabled) {
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
  }

  .add-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .item-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .note-item,
  .reminder-item {
    padding: 0.5rem;
    border: 1px solid var(--color-border-subtle, #1e2028);
    border-radius: 4px;
    background: var(--color-surface-raised, #15171c);
  }

  .note-content {
    font-size: 0.8125rem;
    line-height: 1.5;
    color: var(--color-text, #e6e8ef);
  }

  .note-content :global(code) {
    background: var(--color-surface, #0f1115);
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  .note-content :global(a) {
    color: var(--color-accent, #60a5fa);
  }

  .reminder-label {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    cursor: pointer;
  }

  .reminder-label input[type="checkbox"] {
    accent-color: var(--color-accent, #60a5fa);
    margin-top: 2px;
  }

  .reminder-content {
    font-size: 0.8125rem;
    color: var(--color-text, #e6e8ef);
  }

  .done .reminder-content {
    text-decoration: line-through;
    opacity: 0.5;
  }

  .item-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 0.25rem;
  }

  .age {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .delete-btn {
    padding: 1px 6px;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 3px;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.625rem;
    opacity: 0;
    transition: opacity 150ms;
  }

  .note-item:hover .delete-btn,
  .reminder-item:hover .delete-btn,
  .note-item:focus-within .delete-btn,
  .reminder-item:focus-within .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    color: var(--color-error, #f87171);
    border-color: var(--color-error, #f87171);
  }

  .empty {
    margin: 0;
    padding: 1.5rem;
    text-align: center;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
  }
</style>
