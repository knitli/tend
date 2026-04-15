<!-- P4-E: Quick-switch command palette (Ctrl+K / Cmd+K).

     Full-screen modal overlay listing every session grouped by project, with
     a fuzzy filter (reusing the shared `matchesSessionFilter` predicate) and
     keyboard-only navigation (Arrow keys, Home / End, Enter, Escape).

     The keyboard model mirrors VS Code's quick-open palette:
       - a single flat "candidate" list is built from the filtered + grouped
         sessions (group headers are visual-only, not focusable);
       - the ArrowUp / ArrowDown / Home / End handlers move a roving
         `selectedIndex` through the candidates; Enter activates the
         candidate at that index; Escape closes.
     This deliberately keeps the input always-focused so typing never has to
     re-grab focus after navigating — the selection is purely virtual (rows
     get `aria-selected` + `.selected` styling; `tabindex` stays on the
     input). That's simpler than the `LayoutSwitcher` roving-tabindex
     pattern and better-suited to a palette where the user alternates
     between typing and arrow keys several times per open.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { matchesSessionFilter } from '$lib/util/filterSession';
  import { getProjectColor as resolveProjectColor } from '$lib/util/projectColor';
  import type { SessionSummary } from '$lib/api/sessions';

  interface Props {
    open: boolean;
    onClose: () => void;
    /** Parent decides whether to swap-into-slot or add-new-slot. */
    onActivate: (sessionId: number) => void;
  }

  let { open, onClose, onActivate }: Props = $props();

  let filterText = $state('');
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();
  let rootEl: HTMLDivElement | undefined = $state();
  let modalEl: HTMLDivElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  /** Flat list of all sessions (no status filter — the palette surfaces
   *  every session regardless of state so a user can jump into an ended
   *  session to re-read its scrollback). */
  const filteredSessions = $derived.by(() => {
    const q = filterText;
    return sessionsStore.sessions.filter((s) => {
      const project = projectsStore.byId(s.project_id);
      return matchesSessionFilter(q, s.label, project?.display_name ?? '');
    });
  });

  /** Same grouping pattern as SessionList's `groupedSessions` — grouped by
   *  project id, sorted by project display name, sessions within each
   *  group sorted by last_activity_at descending. */
  const groupedSessions = $derived.by(() => {
    const groups = new Map<number, SessionSummary[]>();
    for (const s of filteredSessions) {
      const list = groups.get(s.project_id);
      if (list) list.push(s);
      else groups.set(s.project_id, [s]);
    }
    for (const list of groups.values()) {
      list.sort((a, b) => b.last_activity_at.localeCompare(a.last_activity_at));
    }
    const entries = Array.from(groups.entries());
    entries.sort((a, b) => {
      const nameA = projectsStore.byId(a[0])?.display_name ?? '';
      const nameB = projectsStore.byId(b[0])?.display_name ?? '';
      return nameA.localeCompare(nameB);
    });
    return entries;
  });

  /** Flat candidate list used by the keyboard model. Must be traversed in
   *  exactly the same order the template renders rows so the visual
   *  `.selected` ring tracks the `selectedIndex`. */
  const candidates = $derived.by<SessionSummary[]>(() => {
    const out: SessionSummary[] = [];
    for (const [, sessions] of groupedSessions) {
      for (const s of sessions) out.push(s);
    }
    return out;
  });

  /** Clamp the selection whenever the filter narrows the list. */
  $effect(() => {
    const n = candidates.length;
    if (n === 0) {
      selectedIndex = 0;
      return;
    }
    if (selectedIndex >= n) selectedIndex = n - 1;
    if (selectedIndex < 0) selectedIndex = 0;
  });

  /** Reset state + focus the input on `open` transitioning true. */
  $effect(() => {
    if (open) {
      filterText = '';
      selectedIndex = 0;
      // Defer focus so the element has been mounted.
      queueMicrotask(() => inputEl?.focus());
    }
  });

  /** Scroll the selected row into view when selectedIndex changes. */
  $effect(() => {
    if (!open) return;
    const idx = selectedIndex;
    queueMicrotask(() => {
      const row = listEl?.querySelector<HTMLElement>(
        `[data-candidate-index="${idx}"]`,
      );
      // jsdom lacks `scrollIntoView`; guard so tests don't trip on it.
      if (row && typeof row.scrollIntoView === 'function') {
        row.scrollIntoView({ block: 'nearest' });
      }
    });
  });

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      const s = candidates[selectedIndex];
      if (s) onActivate(s.id);
      return;
    }
    if (candidates.length === 0) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, candidates.length - 1);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (event.key === 'Home') {
      event.preventDefault();
      selectedIndex = 0;
    } else if (event.key === 'End') {
      event.preventDefault();
      selectedIndex = candidates.length - 1;
    }
  }

  /** Click-outside-the-modal closes. We use a capture-phase handler on the
   *  document so clicks anywhere outside the modal box (including on the
   *  backdrop) dismiss the palette. Clicks *inside* the modal fall through
   *  to their row handlers. */
  function handleDocumentClick(event: MouseEvent): void {
    if (!open) return;
    if (modalEl && !modalEl.contains(event.target as Node)) {
      onClose();
    }
  }

  function statusLabel(status: SessionSummary['status']): string {
    switch (status) {
      case 'working': return 'Working';
      case 'idle': return 'Idle';
      case 'needs_input': return 'Needs Input';
      case 'ended': return 'Ended';
      case 'error': return 'Error';
      default: return status;
    }
  }

  function projectColorFor(projectId: number): string | null {
    return resolveProjectColor(projectsStore.byId(projectId));
  }

  function projectNameFor(projectId: number): string {
    return projectsStore.byId(projectId)?.display_name ?? `Project ${projectId}`;
  }

  /** Map a session id to its candidate index so the row template can
   *  render `aria-selected` and `.selected` without a second pass. */
  const indexBySessionId = $derived.by<Map<number, number>>(() => {
    const map = new Map<number, number>();
    candidates.forEach((s, i) => map.set(s.id, i));
    return map;
  });

  function handleRowClick(sessionId: number): void {
    onActivate(sessionId);
  }

  onMount(() => {
    document.addEventListener('mousedown', handleDocumentClick, true);
  });

  onDestroy(() => {
    document.removeEventListener('mousedown', handleDocumentClick, true);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="palette-overlay"
    bind:this={rootEl}
    role="dialog"
    aria-modal="true"
    aria-label="Quick session switcher"
    tabindex="-1"
    onkeydown={handleKeydown}
  >
    <div class="palette-modal" bind:this={modalEl}>
      <input
        bind:this={inputEl}
        type="text"
        class="palette-input"
        placeholder="Search sessions by label or project…"
        bind:value={filterText}
        aria-label="Filter sessions"
        aria-controls="palette-list"
        aria-activedescendant={candidates[selectedIndex]
          ? `palette-row-${candidates[selectedIndex].id}`
          : undefined}
        autocomplete="off"
        spellcheck="false"
      />

      <div
        class="palette-list"
        id="palette-list"
        role="listbox"
        bind:this={listEl}
        aria-label="Sessions"
      >
        {#if candidates.length === 0}
          <p class="palette-empty">No sessions match.</p>
        {:else}
          {#each groupedSessions as [projectId, sessions] (projectId)}
            {@const color = projectColorFor(projectId)}
            <div
              class="palette-group"
              style={color ? `--project-color: ${color}` : ''}
            >
              <h3 class="palette-group-heading">
                <span class="palette-dot" aria-hidden="true"></span>
                {projectNameFor(projectId)}
              </h3>
              {#each sessions as session (session.id)}
                {@const idx = indexBySessionId.get(session.id) ?? -1}
                {@const isSelected = idx === selectedIndex}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                <div
                  id={`palette-row-${session.id}`}
                  class="palette-row"
                  class:selected={isSelected}
                  role="option"
                  aria-selected={isSelected}
                  tabindex="-1"
                  data-candidate-index={idx}
                  data-session-id={session.id}
                  onclick={() => handleRowClick(session.id)}
                  onmouseenter={() => { if (idx >= 0) selectedIndex = idx; }}
                >
                  <span class="palette-dot" aria-hidden="true"></span>
                  <span class="palette-project">{projectNameFor(session.project_id)}</span>
                  <span class="palette-separator">/</span>
                  <span class="palette-label">{session.label}</span>
                  <span class="palette-spacer"></span>
                  <span class="palette-status palette-status-{session.status.replaceAll('_', '-')}">
                    {statusLabel(session.status)}
                  </span>
                  {#if session.alert}
                    <span
                      class="palette-alert"
                      role="img"
                      aria-label={session.alert.reason ?? 'Needs input'}
                      title={session.alert.reason ?? 'Needs input'}
                    >!</span>
                  {/if}
                </div>
              {/each}
            </div>
          {/each}
        {/if}
      </div>

      <div class="palette-hint" aria-hidden="true">
        <kbd>↑↓</kbd> navigate
        <kbd>↵</kbd> open
        <kbd>Esc</kbd> close
      </div>
    </div>
  </div>
{/if}

<style>
  .palette-overlay {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 10vh;
  }

  .palette-modal {
    width: 560px;
    max-width: calc(100vw - 2rem);
    max-height: 400px;
    display: flex;
    flex-direction: column;
    background: var(--color-surface, #0f1115);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.5rem;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  .palette-input {
    padding: 0.75rem 1rem;
    border: none;
    border-bottom: 1px solid var(--color-border, #2a2d35);
    background: transparent;
    color: var(--color-text, #e6e8ef);
    font-size: 0.875rem;
    font-family: inherit;
    outline: none;
  }

  .palette-input::placeholder {
    color: var(--color-text-muted, #8b8fa3);
  }

  .palette-list {
    flex: 1;
    overflow-y: auto;
    padding: 0.25rem 0;
  }

  .palette-empty {
    margin: 0;
    padding: 1rem;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
    text-align: center;
  }

  .palette-group {
    padding: 0.25rem 0;
  }

  .palette-group-heading {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin: 0;
    padding: 0.375rem 1rem 0.25rem;
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #8b8fa3);
  }

  .palette-group-heading .palette-dot {
    width: 6px;
    height: 6px;
  }

  .palette-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.375rem 1rem;
    cursor: pointer;
    font-size: 0.8125rem;
    color: var(--color-text, #e6e8ef);
    border-left: 2px solid transparent;
  }

  .palette-row.selected {
    background: color-mix(
      in srgb,
      var(--project-color, var(--color-accent, #60a5fa)) 14%,
      var(--color-surface, #0f1115)
    );
    border-left-color: var(--project-color, var(--color-accent, #60a5fa));
  }

  .palette-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    flex-shrink: 0;
  }

  .palette-project {
    color: var(--color-text-muted, #8b8fa3);
    font-weight: 500;
  }

  .palette-separator {
    color: var(--color-text-muted, #8b8fa3);
    opacity: 0.5;
  }

  .palette-label {
    color: var(--color-text, #e6e8ef);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .palette-spacer {
    flex: 1;
  }

  .palette-status {
    font-size: 0.6875rem;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text-muted, #8b8fa3);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .palette-status-working {
    background: color-mix(in srgb, var(--color-accent, #60a5fa) 20%, transparent);
    color: var(--color-accent, #60a5fa);
  }

  .palette-status-needs-input {
    background: var(--color-warning-bg, #3d2e00);
    color: var(--color-warning, #fbbf24);
  }

  .palette-status-error,
  .palette-status-ended {
    background: var(--color-error-bg, rgba(239, 68, 68, 0.1));
    color: var(--color-error, #ef4444);
  }

  .palette-alert {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--color-warning, #fbbf24);
    color: #0f1115;
    font-weight: 700;
    font-size: 0.75rem;
    flex-shrink: 0;
  }

  .palette-hint {
    display: flex;
    gap: 0.75rem;
    padding: 0.375rem 0.75rem;
    border-top: 1px solid var(--color-border, #2a2d35);
    background: var(--color-surface-raised, #15171c);
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .palette-hint kbd {
    padding: 0.0625rem 0.25rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.1875rem;
    background: var(--color-surface, #0f1115);
    font-family: inherit;
    font-size: 0.625rem;
    margin-right: 0.1875rem;
  }
</style>
