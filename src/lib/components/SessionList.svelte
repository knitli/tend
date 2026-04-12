<!-- T064: Session list panel with debounced filter input and
     sessions grouped by project. Renders SessionRow for each session. -->
<script lang="ts">
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import SessionRow from '$lib/components/SessionRow.svelte';
  import type { SessionSummary } from '$lib/api/sessions';

  interface Props {
    selectedProjectId?: number | null;
    onActivateSession?: (session: SessionSummary) => void;
  }

  let { selectedProjectId = null, onActivateSession }: Props = $props();

  let filterText = $state('');
  let debouncedFilter = $state('');
  let showEnded = $state(false);

  // Debounce filter input at 150ms
  $effect(() => {
    const value = filterText;
    const timer = setTimeout(() => { debouncedFilter = value; }, 150);
    return () => clearTimeout(timer);
  });

  /** Sessions filtered by project selection, text filter, and ended toggle. */
  const filteredSessions = $derived.by(() => {
    let result = sessionsStore.sessions;

    // Filter by selected project
    if (selectedProjectId !== null && selectedProjectId !== undefined) {
      result = result.filter((s) => s.project_id === selectedProjectId);
    }

    // Filter out ended sessions unless toggled
    if (!showEnded) {
      result = result.filter(
        (s) => s.status !== 'ended' && s.status !== 'error',
      );
    }

    // Apply text filter against label and project display_name
    const query = debouncedFilter.toLowerCase().trim();
    if (query) {
      result = result.filter((s) => {
        const label = s.label.toLowerCase();
        const project = projectsStore.byId(s.project_id);
        const projectName = project?.display_name.toLowerCase() ?? '';
        return label.includes(query) || projectName.includes(query);
      });
    }

    return result;
  });

  /** Group filtered sessions by project, sorted by project display_name. */
  const groupedSessions = $derived.by(() => {
    const groups = new Map<number, SessionSummary[]>();

    for (const session of filteredSessions) {
      const list = groups.get(session.project_id);
      if (list) {
        list.push(session);
      } else {
        groups.set(session.project_id, [session]);
      }
    }

    // Sort each group by last_activity_at descending
    for (const list of groups.values()) {
      list.sort((a, b) =>
        b.last_activity_at.localeCompare(a.last_activity_at),
      );
    }

    // Return sorted entries by project display_name
    const entries = Array.from(groups.entries());
    entries.sort((a, b) => {
      const nameA = projectsStore.byId(a[0])?.display_name ?? '';
      const nameB = projectsStore.byId(b[0])?.display_name ?? '';
      return nameA.localeCompare(nameB);
    });

    return entries;
  });

  function getProjectName(projectId: number): string {
    return projectsStore.byId(projectId)?.display_name ?? `Project ${projectId}`;
  }

  function handleActivate(session: SessionSummary): void {
    onActivateSession?.(session);
  }
</script>

<div class="session-list" role="region" aria-label="Sessions">
  <div class="session-list-header">
    <input
      type="search"
      placeholder="Filter sessions..."
      bind:value={filterText}
      class="filter-input"
      aria-label="Filter sessions by name or project"
    />
    <label class="toggle-label">
      <input
        type="checkbox"
        bind:checked={showEnded}
        aria-label="Show ended sessions"
      />
      Ended
    </label>
  </div>

  <div class="session-list-body">
    {#if sessionsStore.loading}
      <p class="empty-state">Loading sessions...</p>
    {:else if filteredSessions.length === 0}
      <p class="empty-state">
        {debouncedFilter ? 'No sessions match your filter.' : 'No active sessions.'}
      </p>
    {:else}
      {#each groupedSessions as [projectId, sessions] (projectId)}
        <div class="project-group">
          <h3 class="group-heading">{getProjectName(projectId)}</h3>
          {#each sessions as session (session.id)}
            <SessionRow
              {session}
              projectName={selectedProjectId !== null ? '' : getProjectName(session.project_id)}
              onActivate={handleActivate}
            />
          {/each}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .session-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .session-list-header {
    display: flex;
    align-items: center;
    gap: var(--space-3, 0.75rem);
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .filter-input {
    flex: 1;
    padding: var(--space-2, 0.5rem) var(--space-3, 0.75rem);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
  }

  .filter-input::placeholder {
    color: var(--color-text-muted, #8b8fa3);
  }

  .filter-input:focus {
    outline: none;
    border-color: var(--color-accent, #60a5fa);
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: var(--space-1, 0.25rem);
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    white-space: nowrap;
  }

  .toggle-label input[type="checkbox"] {
    accent-color: var(--color-accent, #60a5fa);
  }

  .session-list-body {
    flex: 1;
    overflow-y: auto;
  }

  .empty-state {
    margin: 0;
    padding: var(--space-6, 1.5rem) var(--space-4, 1rem);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
    text-align: center;
  }

  .project-group {
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .group-heading {
    margin: 0;
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #8b8fa3);
    background: var(--color-surface-raised, #15171c);
    position: sticky;
    top: 0;
    z-index: 1;
  }
</style>
