<!--
  T116: Cross-project overview — groups open reminders by project.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { overviewStore } from '$lib/stores/overview.svelte';
  import { formatAge } from '$lib/util/age';
  import { reminderSetState } from '$lib/api/scratchpad';

  onMount(() => {
    overviewStore.refresh();
  });

  async function handleToggle(reminderId: number, currentState: string) {
    const newState = currentState === 'open' ? 'done' : 'open';
    try {
      await reminderSetState({ id: reminderId, state: newState as 'open' | 'done' });
      // Re-fetch overview to reflect the change.
      await overviewStore.refresh();
    } catch {
      // Silently ignore — state will be stale until next refresh.
    }
  }
</script>

<div class="overview" role="region" aria-label="Cross-project overview">
  <div class="overview-header">
    <h2>Open Reminders</h2>
    <button class="refresh-btn" onclick={() => overviewStore.refresh()} title="Refresh">
      Refresh
    </button>
  </div>

  {#if overviewStore.loading}
    <p class="empty">Loading...</p>
  {:else if overviewStore.groups.length === 0}
    <p class="empty">No open reminders across any project.</p>
  {:else}
    <div class="groups">
      {#each overviewStore.groups as group (group.project.id)}
        <div class="project-group">
          <h3 class="group-heading">{group.project.display_name}</h3>
          <ul class="reminder-list">
            {#each group.open_reminders as reminder (reminder.id)}
              <li class="reminder-item">
                <label class="reminder-label">
                  <input
                    type="checkbox"
                    checked={false}
                    onchange={() => handleToggle(reminder.id, reminder.state)}
                  />
                  <span class="reminder-text">{reminder.content}</span>
                </label>
                <span class="reminder-age">{formatAge(reminder.created_at)}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
  }

  .overview-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--color-border, #2a2d35);
    flex-shrink: 0;
  }

  .overview-header h2 {
    margin: 0;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text, #e6e8ef);
  }

  .refresh-btn {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.6875rem;
  }

  .refresh-btn:hover {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
  }

  .groups {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem;
  }

  .project-group {
    margin-bottom: 0.75rem;
  }

  .group-heading {
    margin: 0 0 0.375rem;
    padding: 0.25rem 0.5rem;
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #8b8fa3);
    background: var(--color-surface-raised, #15171c);
    border-radius: 4px;
  }

  .reminder-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .reminder-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.375rem 0.5rem;
    border-bottom: 1px solid var(--color-border-subtle, #1e2028);
  }

  .reminder-label {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    cursor: pointer;
    flex: 1;
    min-width: 0;
  }

  .reminder-label input[type="checkbox"] {
    accent-color: var(--color-accent, #60a5fa);
    margin-top: 2px;
  }

  .reminder-text {
    font-size: 0.8125rem;
    color: var(--color-text, #e6e8ef);
  }

  .reminder-age {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
    white-space: nowrap;
    flex-shrink: 0;
    margin-left: 0.5rem;
  }

  .empty {
    margin: 0;
    padding: 1.5rem;
    text-align: center;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
  }
</style>
