<!-- T064: Session list panel with debounced filter input and
     sessions grouped by project. Renders SessionRow for each session. -->
<script lang="ts">
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { matchesSessionFilter } from '$lib/util/filterSession';
  import { getProjectColor as resolveProjectColor } from '$lib/util/projectColor';
  import SessionRow from '$lib/components/SessionRow.svelte';
  import type { SessionSummary } from '$lib/api/sessions';
  import { dndzone, type DndEvent } from 'svelte-dnd-action';

  interface Props {
    selectedProjectId?: number | null;
    missingSessions?: Set<number>;
    /** Session ids currently visible in a pane slot. In Phase 1 this is always a
     *  set of zero or one — Phase 4 expands it to the full visible-slot set. */
    activeSessionIds?: Set<number>;
    onActivateSession?: (session: SessionSummary) => void;
    onSpawnSession?: () => void;
    /** P4-D: keyboard-accessible fallback for the drag-to-pane gesture. When
     *  provided, each SessionRow renders a `⊞` button that calls this with the
     *  session so a non-mouse user can add it to a pane slot. When omitted,
     *  no `⊞` button is rendered. */
    onOpenInSlot?: (session: SessionSummary) => void;
  }

  let {
    selectedProjectId = null,
    missingSessions = new Set(),
    activeSessionIds = new Set(),
    onActivateSession,
    onSpawnSession,
    onOpenInSlot,
  }: Props = $props();

  /** P4-D: svelte-dnd-action item shape. Each item needs a string `id` key
   *  (we use `dnd-session-<id>`) plus the payload our drop target cares
   *  about — the session id. Grouping is preserved by rendering one
   *  `use:dndzone` per project group (nested zones), all sharing the
   *  `type: 'session-source'` so the AddSlotZone accepts them equally.
   *  Each zone is source-only (`dropFromOthersDisabled: true`) so drags
   *  from one group cannot reparent into another. */
  type DnDSessionItem = { id: string; sessionId: number };

  let filterInputEl: HTMLInputElement | undefined = $state();

  /** Focus the filter input. Called from the parent page when the `/`
   *  keyboard shortcut is pressed. Exported via Svelte 5 runes module API. */
  export function focusFilter(): void {
    filterInputEl?.focus();
    filterInputEl?.select();
  }

  /** P1-B: handle AlertBar "Go to" → scroll a session row into view.
   *  TODO(phase-4): when the Workspace tab mounts a second SessionList, scope
   *  this listener to the list container (or add instance disambiguation to
   *  the event payload) so both lists don't scroll independently and fight
   *  over `document.querySelector`. */
  $effect(() => {
    function handleScrollTo(event: Event): void {
      const detail = (event as CustomEvent<{ sessionId: number }>).detail;
      if (!detail) return;
      const el = document.querySelector(`[data-session-id="${detail.sessionId}"]`);
      const behavior = window.matchMedia?.('(prefers-reduced-motion: reduce)')?.matches
        ? 'auto'
        : 'smooth';
      el?.scrollIntoView({ behavior, block: 'nearest' });
    }
    window.addEventListener('tend:session-scroll-to', handleScrollTo);
    return () => {
      window.removeEventListener('tend:session-scroll-to', handleScrollTo);
    };
  });

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

    // Apply text filter against label and project display_name (M7: shared predicate).
    if (debouncedFilter) {
      result = result.filter((s) => {
        const project = projectsStore.byId(s.project_id);
        return matchesSessionFilter(debouncedFilter, s.label, project?.display_name ?? '');
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

  /** P4-D: build per-group DnD item arrays keyed by project id. Each array
   *  is a stable reference for svelte-dnd-action's `items` prop. The
   *  corresponding `dndGroupItems` $state reflects transient consider-phase
   *  mutations; we restore it to the canonical groupedSessions slice on
   *  finalize so the list never actually reorders. */
  const groupedDndItems = $derived.by(() => {
    const map = new Map<number, DnDSessionItem[]>();
    for (const [projectId, sessions] of groupedSessions) {
      map.set(projectId, sessions.map((s) => ({ id: `dnd-session-${s.id}`, sessionId: s.id })));
    }
    return map;
  });

  /** Per-group mutable items the zone writes back during a drag. Keyed by
   *  project id. */
  let dndGroupItems = $state<Map<number, DnDSessionItem[]>>(new Map());
  $effect(() => {
    // Rebuild the mutable map from the canonical derived whenever the
    // grouping changes (new session arrived, filter changed, etc).
    const fresh = new Map<number, DnDSessionItem[]>();
    for (const [projectId, items] of groupedDndItems) {
      fresh.set(projectId, items.slice());
    }
    dndGroupItems = fresh;
  });

  function dndItemsForProject(projectId: number): DnDSessionItem[] {
    return dndGroupItems.get(projectId) ?? [];
  }

  function makeConsiderHandler(projectId: number) {
    return (e: CustomEvent<DndEvent<DnDSessionItem>>) => {
      const next = new Map(dndGroupItems);
      next.set(projectId, e.detail.items);
      dndGroupItems = next;
    };
  }

  function makeFinalizeHandler(projectId: number) {
    return (_e: CustomEvent<DndEvent<DnDSessionItem>>) => {
      // Source-only zones: restore canonical items regardless of outcome.
      // If the drop landed in a sibling zone, that zone handles the side
      // effect (e.g. addSlot). If the drop landed here or outside, the
      // list order must be preserved.
      const canonical = groupedDndItems.get(projectId) ?? [];
      const next = new Map(dndGroupItems);
      next.set(projectId, canonical.slice());
      dndGroupItems = next;
    };
  }

  function getProjectName(projectId: number): string {
    return projectsStore.byId(projectId)?.display_name ?? `Project ${projectId}`;
  }

  /** Phase 2-C: look up the project's palette colour for heading/row tinting.
   *  Returns `null` when absent (or invalid) so the component can fall back
   *  to the global `--color-accent` via CSS `var()`. Delegates hex validation
   *  to the shared helper so a malformed DB value never reaches an inline
   *  `style="--project-color: ..."` (CSS var values skip Svelte escaping). */
  function getProjectColor(projectId: number): string | null {
    return resolveProjectColor(projectsStore.byId(projectId));
  }

  function handleActivate(session: SessionSummary): void {
    onActivateSession?.(session);
  }

  /** Project used for the empty-state copy hint: selected one if any,
   *  otherwise the first active project. */
  const hintProject = $derived.by(() => {
    if (selectedProjectId !== null && selectedProjectId !== undefined) {
      return projectsStore.byId(selectedProjectId) ?? null;
    }
    return projectsStore.activeProjects[0] ?? null;
  });

  const hintCommand = $derived(
    hintProject
      ? `tend run --project "${hintProject.canonical_path}" -- claude`
      : 'tend run -- claude',
  );

  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  async function copyHint(): Promise<void> {
    try {
      await navigator.clipboard.writeText(hintCommand);
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => { copied = false; }, 1500);
    } catch {
      // Clipboard API may be unavailable (non-secure context); fail silently.
    }
  }
</script>

<div class="session-list" role="region" aria-label="Sessions">
  <div class="session-list-header">
    <input
      bind:this={filterInputEl}
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
    {#if onSpawnSession}
      <button
        type="button"
        class="new-session-btn"
        onclick={() => onSpawnSession?.()}
        disabled={projectsStore.activeProjects.length === 0}
        title={projectsStore.activeProjects.length === 0
          ? 'Add a project first'
          : 'Start a new session'}
        aria-label="Start a new session"
      >
        + New
      </button>
    {/if}
  </div>

  <div class="session-list-body">
    {#if sessionsStore.loading}
      <p class="empty-state">Loading sessions...</p>
    {:else if filteredSessions.length === 0}
      {#if debouncedFilter}
        <p class="empty-state">No sessions match your filter.</p>
      {:else}
        <div class="empty-state empty-state-hint">
          <p class="hint-lead">No active sessions yet.</p>
          {#if projectsStore.activeProjects.length === 0}
            <p class="hint-body">
              Add a project in the sidebar first.
            </p>
          {:else}
            <p class="hint-body">
              Use <strong>+ New</strong> above, or run from a terminal:
            </p>
          {/if}
          <div class="hint-command">
            <code>{hintCommand}</code>
            <button
              type="button"
              class="btn-copy"
              onclick={copyHint}
              title="Copy command"
              aria-label="Copy command"
            >
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
        </div>
      {/if}
    {:else}
      {#each groupedSessions as [projectId, sessions] (projectId)}
        {@const projectColor = getProjectColor(projectId)}
        {@const groupItems = dndItemsForProject(projectId)}
        {@const sessionById = new Map(sessions.map((session) => [session.id, session]))}
        <div
          class="project-group"
          style={projectColor ? `--project-color: ${projectColor}` : ''}
        >
          <h3 class="group-heading">{getProjectName(projectId)}</h3>
          <div
            class="project-group-items"
            use:dndzone={{
              items: groupItems,
              type: 'session-source',
              dropFromOthersDisabled: true,
              flipDurationMs: 0,
              morphDisabled: true,
              dropTargetStyle: {},
            }}
            onconsider={makeConsiderHandler(projectId)}
            onfinalize={makeFinalizeHandler(projectId)}
          >
            {#each groupItems as item (item.id)}
              {@const session = sessionById.get(item.sessionId)}
              {#if session}
                <SessionRow
                  {session}
                  projectName={selectedProjectId !== null ? '' : getProjectName(session.project_id)}
                  missing={missingSessions.has(session.id)}
                  active={activeSessionIds.has(session.id)}
                  anyActive={activeSessionIds.size > 0}
                  {projectColor}
                  onActivate={handleActivate}
                  {onOpenInSlot}
                />
              {/if}
            {/each}
          </div>
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

  .new-session-btn {
    padding: var(--space-1, 0.25rem) var(--space-3, 0.75rem);
    border: 1px solid var(--color-accent, #60a5fa);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
    font-size: 0.75rem;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    white-space: nowrap;
    transition: background 120ms;
  }

  .new-session-btn:hover:not(:disabled) {
    background: var(--color-accent-hover, #93c5fd);
  }

  .new-session-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
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

  .empty-state-hint {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: var(--space-3, 0.75rem);
  }

  .hint-lead {
    margin: 0;
    color: var(--color-text, #e6e8ef);
    font-size: 0.875rem;
    font-weight: 500;
  }

  .hint-body {
    margin: 0;
    line-height: 1.5;
  }

  .hint-body strong {
    color: var(--color-text, #e6e8ef);
    font-weight: 600;
  }

  .hint-command {
    display: flex;
    align-items: stretch;
    gap: var(--space-2, 0.5rem);
    background: var(--color-surface-raised, #15171c);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    padding: var(--space-2, 0.5rem);
  }

  .hint-command code {
    flex: 1;
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.75rem;
    color: var(--color-text, #e6e8ef);
    white-space: nowrap;
    overflow-x: auto;
    align-self: center;
  }

  .btn-copy {
    padding: var(--space-1, 0.25rem) var(--space-3, 0.75rem);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-surface, #0f1115);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.75rem;
    font-family: inherit;
    cursor: pointer;
    transition: background 150ms, color 150ms, border-color 150ms;
    flex-shrink: 0;
  }

  .btn-copy:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
    border-color: var(--color-accent, #60a5fa);
  }

  .project-group {
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  /* P4-D: svelte-dnd-action uses the direct children of the zone as
     draggables. Keeping the items container as a plain flex column so
     the row divider lines inside SessionRow still align edge-to-edge. */
  .project-group-items {
    display: flex;
    flex-direction: column;
  }

  /* Phase 2-C: group headings carry a 2 px left-strip in the project colour
     so adjacent groups are visually distinct even before the row colours
     kick in. Falls back to the global accent when `--project-color` is
     unset (pre-Phase-2 projects). */
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
    border-left: 2px solid var(--project-color, var(--color-accent, #60a5fa));
  }
</style>
