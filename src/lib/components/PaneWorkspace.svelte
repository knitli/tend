<!--
  PaneWorkspace — responsive grid container for session panes (Phase 4-B, reworked).

  Replaces the paneforge horizontal PaneGroup with a CSS grid that:
  - 1-3 slots: single column, stacked vertically (each full width)
  - 4+ slots: two columns (50% width each)
  - All rows share height equally (1fr)
  - Drag-and-drop reorder between grid positions
  - Overflow popover for slots that exceed comfortable viewing

  This is a controlled component: the parent (`+page.svelte`) owns the
  `slots` state and wires the mutation callbacks.
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import PaneSlot from '$lib/components/PaneSlot.svelte';
  import AddSlotZone from '$lib/components/AddSlotZone.svelte';
  import { sessionSetVisible } from '$lib/api/sessions';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { getProjectColor } from '$lib/util/projectColor';
  import type { PaneSlot as PaneSlotType } from '$lib/types/pane';

  /** Minimum comfortable height for a single pane's terminal row. */
  const MIN_PANE_HEIGHT_PX = 180;

  interface Props {
    slots: PaneSlotType[];
    /** Persisted pane sizes — kept for API compat but not used by grid layout. */
    paneSizes?: number[];
    highlightedSessionId?: number | null;
    highlightToken?: number;
    onSlotClose: (sessionId: number) => void;
    onSlotFocus: (sessionId: number) => void;
    onResize?: (sizes: number[]) => void;
    onDropSession?: (sessionId: number) => void;
    onReorderSlots?: (next: PaneSlotType[]) => void;
    onRestartSlot?: (slotSessionId: number) => Promise<number | null>;
  }

  let {
    slots,
    paneSizes,
    highlightedSessionId = null,
    highlightToken = 0,
    onSlotClose,
    onSlotFocus,
    onResize,
    onDropSession,
    onReorderSlots,
    onRestartSlot,
  }: Props = $props();

  // --- Drag-to-reorder state ---
  let reorderDragSessionId = $state<number | null>(null);
  let reorderDropTargetSessionId = $state<number | null>(null);

  function reorderSlots(fromSessionId: number, toSessionId: number): void {
    if (fromSessionId === toSessionId) return;
    const fromIdx = slots.findIndex((s) => s.session_id === fromSessionId);
    const toIdx = slots.findIndex((s) => s.session_id === toSessionId);
    if (fromIdx === -1 || toIdx === -1) return;
    const next = slots.slice();
    const [moved] = next.splice(fromIdx, 1);
    next.splice(toIdx, 0, moved);
    onReorderSlots?.(next.map((s, i) => ({ ...s, order: i })));
  }

  function handleMoveLeft(sessionId: number): void {
    const idx = slots.findIndex((s) => s.session_id === sessionId);
    if (idx <= 0) return;
    const next = slots.slice();
    [next[idx - 1], next[idx]] = [next[idx], next[idx - 1]];
    onReorderSlots?.(next.map((s, i) => ({ ...s, order: i })));
  }

  function handleMoveRight(sessionId: number): void {
    const idx = slots.findIndex((s) => s.session_id === sessionId);
    if (idx === -1 || idx >= slots.length - 1) return;
    const next = slots.slice();
    [next[idx], next[idx + 1]] = [next[idx + 1], next[idx]];
    onReorderSlots?.(next.map((s, i) => ({ ...s, order: i })));
  }

  function handleReorderDragStart(sessionId: number, event: DragEvent): void {
    if (!event.dataTransfer) return;
    reorderDragSessionId = sessionId;
    reorderDropTargetSessionId = null;
    event.dataTransfer.setData('application/x-tend-pane-slot', String(sessionId));
    event.dataTransfer.effectAllowed = 'move';

    // Custom drag ghost
    const sess = sessionsStore.byId(sessionId);
    const proj = sess ? (projectsStore.byId(sess.project_id) ?? null) : null;
    const color = getProjectColor(proj) ?? '#60a5fa';
    const labelText = sess?.label ?? `Session ${sessionId}`;
    const projectText = proj?.display_name ?? '';

    const ghost = document.createElement('div');
    ghost.style.cssText = [
      'position:fixed', 'top:-200px', 'left:-200px',
      'display:inline-flex', 'align-items:center', 'gap:6px',
      'padding:4px 10px 4px 8px', 'background:#0f1115',
      'border:1px solid #2a2d35', `border-left:3px solid ${color}`,
      'border-radius:4px', 'color:#e6e8ef', 'font:12px/1.4 inherit',
      'box-shadow:0 2px 8px rgba(0,0,0,.4)', 'white-space:nowrap',
      'z-index:-1', 'pointer-events:none',
    ].join(';');

    const dot = document.createElement('span');
    dot.style.cssText = `display:inline-block;width:8px;height:8px;border-radius:50%;background:${color};flex-shrink:0`;
    ghost.appendChild(dot);

    const titleWrap = document.createElement('span');
    if (projectText) {
      const projEl = document.createElement('strong');
      projEl.textContent = projectText;
      titleWrap.appendChild(projEl);
      titleWrap.appendChild(document.createTextNode(' · '));
    }
    const labelEl = document.createElement('span');
    labelEl.style.color = '#8b8fa3';
    labelEl.textContent = labelText;
    titleWrap.appendChild(labelEl);
    ghost.appendChild(titleWrap);

    document.body.appendChild(ghost);
    event.dataTransfer.setDragImage(ghost, 20, 14);
    requestAnimationFrame(() => ghost.remove());
  }

  function handleReorderDragEnd(): void {
    reorderDragSessionId = null;
    reorderDropTargetSessionId = null;
  }

  function handleReorderDragOver(targetSessionId: number): void {
    reorderDropTargetSessionId = targetSessionId;
  }

  function handleReorderDragLeave(): void {
    reorderDropTargetSessionId = null;
  }

  function handleReorderDrop(targetSessionId: number): void {
    if (reorderDragSessionId === null) return;
    reorderSlots(reorderDragSessionId, targetSessionId);
    reorderDragSessionId = null;
    reorderDropTargetSessionId = null;
  }

  function getDropIndicator(slotSessionId: number): 'before' | 'after' | null {
    if (reorderDragSessionId === null || reorderDropTargetSessionId !== slotSessionId) return null;
    if (reorderDragSessionId === slotSessionId) return null;
    const fromIdx = slots.findIndex((s) => s.session_id === reorderDragSessionId);
    const toIdx = slots.findIndex((s) => s.session_id === slotSessionId);
    if (fromIdx === -1 || toIdx === -1) return null;
    return fromIdx < toIdx ? 'after' : 'before';
  }

  // --- Container measurement ---
  let containerEl: HTMLDivElement | undefined = $state();
  let containerHeight = $state(0);

  /** How many slots fit comfortably at MIN_PANE_HEIGHT_PX per row.
   *  The grid uses 2 columns for 4+ slots, so max visible is based on
   *  how many rows fit × columns. */
  const maxVisibleSlots = $derived.by(() => {
    if (containerHeight <= 0) return 4; // sensible default before measurement
    // With 2-column layout, each row is shared by 2 slots
    const cols = slots.length >= 4 ? 2 : 1;
    const maxRows = Math.max(1, Math.floor(containerHeight / MIN_PANE_HEIGHT_PX));
    return Math.max(1, maxRows * cols);
  });

  const visibleSlots = $derived(slots.slice(0, maxVisibleSlots));
  const overflowCount = $derived(Math.max(0, slots.length - visibleSlots.length));

  /** Number of grid columns based on visible slot count. */
  const gridCols = $derived(visibleSlots.length >= 4 ? 2 : 1);

  // Call sessionSetVisible whenever the set of VISIBLE slot ids changes.
  const visibleKey = $derived(
    visibleSlots
      .map((s) => s.session_id)
      .sort((a, b) => a - b)
      .join(','),
  );
  $effect(() => {
    visibleKey;
    const ids = untrack(() => visibleSlots.map((s) => s.session_id));
    sessionSetVisible({ sessionIds: ids }).catch(() => {});
  });

  // Measure container
  let ro: ResizeObserver | null = null;
  onMount(() => {
    if (!containerEl) return;
    containerHeight = containerEl.clientHeight;
    if (typeof ResizeObserver !== 'undefined') {
      ro = new ResizeObserver((entries) => {
        for (const entry of entries) {
          containerHeight = entry.contentRect.height;
        }
      });
      ro.observe(containerEl);
    }
  });

  onDestroy(() => {
    ro?.disconnect();
    ro = null;
  });

  // --- Overflow popover ---
  let overflowOpen = $state(false);
  let overflowTriggerEl: HTMLButtonElement | undefined = $state();
  let overflowPopoverEl: HTMLDivElement | undefined = $state();
  let overflowFocusedIndex = $state(-1);

  const overflowSlots = $derived.by(() => {
    if (overflowCount === 0) return [] as Array<{
      slot: PaneSlotType;
      absoluteIndex: number;
      projectName: string;
      sessionLabel: string;
      statusLabel: string;
      statusClass: string;
      color: string | undefined;
    }>;
    return slots.slice(maxVisibleSlots).map((slot, i) => {
      const absoluteIndex = maxVisibleSlots + i;
      const session = sessionsStore.byId(slot.session_id) ?? null;
      const project = session ? projectsStore.byId(session.project_id) ?? null : null;
      const ghost = slot.ghost_data;
      let color: string | undefined;
      let projectName: string;
      let sessionLabel: string;
      let statusLabel = '';
      let statusClass = '';
      if (session) {
        color = getProjectColor(project) ?? undefined;
        projectName = project?.display_name ?? 'Project';
        sessionLabel = session.label;
        switch (session.status) {
          case 'working': statusLabel = 'Working'; break;
          case 'idle': statusLabel = 'Idle'; break;
          case 'needs_input': statusLabel = 'Needs Input'; break;
          case 'ended': statusLabel = 'Ended'; break;
          case 'error': statusLabel = 'Error'; break;
          default: statusLabel = session.status;
        }
        statusClass = `status-${session.status.replaceAll('_', '-')}`;
      } else if (ghost) {
        color = ghost.project_color ?? undefined;
        const ghostProject = projectsStore.byId(ghost.project_id) ?? null;
        projectName = ghostProject?.display_name ?? `Project #${ghost.project_id}`;
        sessionLabel = ghost.label;
        statusLabel = 'Ended';
        statusClass = 'status-ended';
      } else {
        color = undefined;
        projectName = 'Project';
        sessionLabel = `Session #${slot.session_id}`;
        statusLabel = 'Ended';
        statusClass = 'status-ended';
      }
      return { slot, absoluteIndex, projectName, sessionLabel, statusLabel, statusClass, color };
    });
  });

  function toggleOverflow(): void {
    if (overflowOpen) { closeOverflow(); } else {
      overflowOpen = true;
      overflowFocusedIndex = -1;
    }
  }

  function closeOverflow(): void {
    overflowOpen = false;
    overflowFocusedIndex = -1;
    overflowTriggerEl?.focus();
  }

  function bringIntoView(absoluteIndex: number): void {
    if (!onReorderSlots) return;
    const lastVisible = maxVisibleSlots - 1;
    if (absoluteIndex <= lastVisible || absoluteIndex >= slots.length) return;
    const next = slots.slice();
    [next[lastVisible], next[absoluteIndex]] = [next[absoluteIndex], next[lastVisible]];
    onReorderSlots(next.map((s, i) => ({ ...s, order: i })));
    closeOverflow();
  }

  function handleOverflowDocumentClick(event: MouseEvent): void {
    if (!overflowOpen) return;
    const target = event.target as Node;
    if (overflowPopoverEl?.contains(target)) return;
    if (overflowTriggerEl?.contains(target)) return;
    closeOverflow();
  }

  function handleOverflowKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') { event.preventDefault(); closeOverflow(); return; }
    if (overflowSlots.length === 0) return;
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      overflowFocusedIndex = Math.min(
        overflowFocusedIndex < 0 ? 0 : overflowFocusedIndex + 1,
        overflowSlots.length - 1,
      );
      focusOverflowItem();
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      overflowFocusedIndex = Math.max(overflowFocusedIndex - 1, 0);
      focusOverflowItem();
    } else if (event.key === 'Home') {
      event.preventDefault();
      overflowFocusedIndex = 0;
      focusOverflowItem();
    } else if (event.key === 'End') {
      event.preventDefault();
      overflowFocusedIndex = overflowSlots.length - 1;
      focusOverflowItem();
    }
  }

  function focusOverflowItem(): void {
    if (overflowFocusedIndex < 0 || !overflowPopoverEl) return;
    const items = overflowPopoverEl.querySelectorAll<HTMLButtonElement>('.pane-overflow-item');
    items[overflowFocusedIndex]?.focus();
  }

  $effect(() => {
    if (!overflowOpen) return;
    if (overflowCount === 0) {
      untrack(() => { overflowOpen = false; overflowFocusedIndex = -1; });
    }
  });

  onMount(() => {
    document.addEventListener('click', handleOverflowDocumentClick, true);
  });

  onDestroy(() => {
    document.removeEventListener('click', handleOverflowDocumentClick, true);
  });
</script>

<div
  class="pane-workspace"
  bind:this={containerEl}
  data-overflow-count={overflowCount}
  data-visible-count={visibleSlots.length}
  data-slot-count={slots.length}
>
  {#if visibleSlots.length === 0}
    <div class="pane-workspace-empty">
      <p class="muted">No sessions open. Pick one from the list to get started.</p>
    </div>
    {#if onDropSession}
      <AddSlotZone onDrop={onDropSession} />
    {/if}
  {:else}
    <div
      class="pane-grid"
      style="grid-template-columns: repeat({gridCols}, 1fr);"
    >
      {#each visibleSlots as slot, i (slot.session_id)}
        <div
          class="pane-grid-cell"
          class:being-dragged={reorderDragSessionId === slot.session_id}
        >
          <PaneSlot
            sessionId={slot.session_id}
            highlighted={highlightedSessionId === slot.session_id}
            {highlightToken}
            ghostData={slot.ghost_data}
            onRestart={onRestartSlot
              ? () => onRestartSlot(slot.session_id)
              : undefined}
            onClose={() => onSlotClose(slot.session_id)}
            onFocus={() => onSlotFocus(slot.session_id)}
            onMoveLeft={onReorderSlots && slots.length > 1 ? () => handleMoveLeft(slot.session_id) : undefined}
            onMoveRight={onReorderSlots && slots.length > 1 ? () => handleMoveRight(slot.session_id) : undefined}
            canMoveLeft={i > 0}
            canMoveRight={i < visibleSlots.length - 1}
            onReorderDragStart={onReorderSlots && slots.length > 1
              ? (e) => handleReorderDragStart(slot.session_id, e)
              : undefined}
            onReorderDragEnd={onReorderSlots && slots.length > 1
              ? handleReorderDragEnd
              : undefined}
            onReorderDragOver={onReorderSlots && slots.length > 1
              ? () => handleReorderDragOver(slot.session_id)
              : undefined}
            onReorderDragLeave={onReorderSlots && slots.length > 1
              ? handleReorderDragLeave
              : undefined}
            onReorderDrop={onReorderSlots && slots.length > 1
              ? () => handleReorderDrop(slot.session_id)
              : undefined}
            isBeingDragged={reorderDragSessionId === slot.session_id}
            dropIndicator={getDropIndicator(slot.session_id)}
          />
        </div>
      {/each}
    </div>
    {#if onDropSession}
      <AddSlotZone onDrop={onDropSession} />
    {/if}
  {/if}

  {#if overflowCount > 0}
    <div class="pane-overflow">
      <button
        type="button"
        class="pane-overflow-trigger"
        bind:this={overflowTriggerEl}
        onclick={toggleOverflow}
        aria-expanded={overflowOpen}
        aria-haspopup="menu"
        aria-label="{overflowCount} session{overflowCount === 1 ? '' : 's'} don't fit — click to choose"
        title="{overflowCount} session{overflowCount === 1 ? '' : 's'} don't fit"
      >
        +{overflowCount} more
      </button>

      {#if overflowOpen}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          class="pane-overflow-popover"
          bind:this={overflowPopoverEl}
          role="menu"
          tabindex="-1"
          aria-label="Hidden sessions"
          onkeydown={handleOverflowKeydown}
        >
          <div class="pane-overflow-header">Hidden sessions</div>
          {#each overflowSlots as entry, i (entry.slot.session_id)}
            <button
              type="button"
              class="pane-overflow-item"
              role="menuitem"
              tabindex={overflowFocusedIndex === i ? 0 : -1}
              onclick={() => bringIntoView(entry.absoluteIndex)}
              style={entry.color ? `--project-color: ${entry.color}` : ''}
              title="Bring {entry.projectName} / {entry.sessionLabel} into view"
            >
              <span class="pane-overflow-dot" aria-hidden="true"></span>
              <span class="pane-overflow-title">
                <span class="pane-overflow-project">{entry.projectName}</span>
                <span class="pane-overflow-separator">/</span>
                <span class="pane-overflow-label">{entry.sessionLabel}</span>
              </span>
              {#if entry.statusLabel}
                <span class="badge {entry.statusClass}">{entry.statusLabel}</span>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .pane-workspace {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    position: relative;
  }

  /* Responsive grid replacing the old horizontal paneforge layout.
     - 1-3 slots: 1 column (full width, stacked vertically)
     - 4+ slots: 2 columns (each 50% width)
     Grid-template-columns is set via inline style based on slot count. */
  .pane-grid {
    display: grid;
    grid-auto-rows: 1fr;
    gap: 2px;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .pane-grid-cell {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 2px;
    transition: opacity 150ms;
  }

  .pane-grid-cell.being-dragged {
    opacity: 0.4;
  }

  .pane-workspace-empty {
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    padding: var(--space-6, 1.5rem);
  }

  .muted {
    margin: 0;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.875rem;
  }

  /* Overflow popover */
  .pane-overflow {
    position: absolute;
    top: 4px;
    right: 8px;
    z-index: 40;
  }

  .pane-overflow-trigger {
    padding: 0.125rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: var(--color-surface, #0f1115);
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.6875rem;
    font-weight: 600;
    line-height: 1.4;
    white-space: nowrap;
  }

  .pane-overflow-trigger:hover,
  .pane-overflow-trigger[aria-expanded="true"] {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
    border-color: var(--color-accent, #60a5fa);
  }

  .pane-overflow-popover {
    position: absolute;
    top: 100%;
    right: 0;
    z-index: 100;
    min-width: 260px;
    max-width: 360px;
    margin-top: 0.25rem;
    padding: 0.25rem;
    background: var(--color-surface, #0f1115);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    max-height: 60vh;
    overflow-y: auto;
  }

  .pane-overflow-header {
    padding: 0.25rem 0.5rem 0.375rem;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .pane-overflow-item {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    width: 100%;
    padding: 0.375rem 0.5rem;
    border: none;
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text, #e6e8ef);
    cursor: pointer;
    text-align: left;
    font-size: 0.75rem;
    min-width: 0;
  }

  .pane-overflow-item:hover,
  .pane-overflow-item:focus-visible {
    background: var(--color-surface-hover, #1a1d25);
    outline: 1px solid var(--color-accent, #60a5fa);
  }

  .pane-overflow-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    flex-shrink: 0;
  }

  .pane-overflow-title {
    display: inline-flex;
    align-items: baseline;
    gap: 0.25rem;
    min-width: 0;
    flex: 1;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .pane-overflow-project {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }

  .pane-overflow-separator {
    color: var(--color-text-muted, #8b8fa3);
    opacity: 0.6;
    flex-shrink: 0;
  }

  .pane-overflow-label {
    color: var(--color-text-muted, #8b8fa3);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.6875rem;
    font-weight: 500;
    line-height: 1.4;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .status-working { background: var(--color-success-bg, #064e3b); color: var(--color-success, #34d399); }
  .status-idle { background: var(--color-info-bg, #1e3a5f); color: var(--color-info, #60a5fa); }
  .status-needs-input { background: var(--color-warning-bg, #3d2e00); color: var(--color-warning, #fbbf24); }
  .status-ended { background: var(--color-muted-bg, #1e2028); color: var(--color-text-muted, #8b8fa3); }
  .status-error { background: var(--color-error-bg, #4c0519); color: var(--color-error, #f87171); }
</style>
