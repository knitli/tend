<!--
  PaneWorkspace — horizontal multi-slot container (Phase 4-B).

  Renders a paneforge <PaneGroup> containing one <Pane> per visible slot.
  This is a controlled component: the parent (`+page.svelte`) owns the
  `slots` state and wires the mutation callbacks. The workspace is purely
  responsible for:

  1. Laying the slots out horizontally via paneforge
  2. Calling `sessionSetVisible` whenever the slot id set changes so the
     backend stops forwarding PTY output for sessions no pane can render
  3. Tracking container width and computing `overflowCount` (slots that
     would fall below `MIN_PANE_WIDTH_PX`). Phase 4-G will surface these
     via a popover; for Phase 4-B we just clip the render list and still
     report them in `sessionSetVisible` so backlog keeps replaying.
  4. Threading highlight (flash) tokens into only the matching slot so
     flashes don't bleed across panes.

  paneforge `minSize` is a percentage of the group's size, not pixels —
  we convert `MIN_PANE_WIDTH_PX` to a percentage against the current
  container width and clamp it to a sane [5, 50] range so one pane with
  a huge minSize can't lock the others out. The real enforcement of
  "don't squeeze below 520 px" happens via `maxVisibleSlots` (the
  overflow path), not via `minSize`.
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import { PaneGroup, Pane, PaneResizer } from 'paneforge';
  import PaneSlot from '$lib/components/PaneSlot.svelte';
  import AddSlotZone from '$lib/components/AddSlotZone.svelte';
  import { sessionSetVisible } from '$lib/api/sessions';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { getProjectColor } from '$lib/util/projectColor';
  import type { PaneSlot as PaneSlotType } from '$lib/types/pane';

  /** Phase 4-B: minimum comfortable width for a single pane's terminal.
   *  Below this, further slots are hidden behind the (future) overflow
   *  popover. Matches `MIN_PANE_WIDTH` in specs/002-adaptive-ui/plan.md. */
  const MIN_PANE_WIDTH_PX = 520;

  interface Props {
    slots: PaneSlotType[];
    /** Persisted pane sizes (percent per slot). If omitted or length
     *  mismatches, each pane distributes evenly. */
    paneSizes?: number[];
    /** Most-recently-activated session id (for pane border flash). */
    highlightedSessionId?: number | null;
    /** Monotonic re-activation token. Threaded only into the matching
     *  slot so other slots don't flash. */
    highlightToken?: number;
    onSlotClose: (sessionId: number) => void;
    onSlotFocus: (sessionId: number) => void;
    onResize?: (sizes: number[]) => void;
    /** P4-D: fired when a session is dropped onto the trailing AddSlotZone
     *  (or when the `⊞` SessionRow button is clicked in +page.svelte).
     *  The parent is responsible for the actual `slots` mutation — this
     *  component stays controlled. */
    onDropSession?: (sessionId: number) => void;
    /** P4-D: fired when the user reorders panes via the drag handle on
     *  the pane header (HTML5 native drag) or the `‹` / `›` keyboard
     *  buttons. Receives the full new slot array in its final order. */
    onReorderSlots?: (next: PaneSlotType[]) => void;
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
  }: Props = $props();

  /** P4-D: id of the slot currently being dragged for reorder. We carry
   *  this via component state rather than round-tripping through the
   *  DataTransfer payload (which is often unreadable outside `drop`). */
  let reorderDragSessionId = $state<number | null>(null);

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
    // Custom MIME type so AddSlotZone / session-source zones don't pick
    // this drag up, and peer slots recognise it in ondragover.
    event.dataTransfer.setData('application/x-tend-pane-slot', String(sessionId));
    event.dataTransfer.effectAllowed = 'move';
  }

  function handleReorderDrop(targetSessionId: number): void {
    if (reorderDragSessionId === null) return;
    reorderSlots(reorderDragSessionId, targetSessionId);
    reorderDragSessionId = null;
  }

  let containerEl: HTMLDivElement | undefined = $state();
  let containerWidth = $state(0);

  /** How many slots comfortably fit at MIN_PANE_WIDTH_PX. Always at least 1
   *  (even in a window so narrow that `floor(w / 520) == 0` we still show
   *  one pane rather than an empty workspace). Before first measurement,
   *  default to 1 so we don't briefly mount all panes. */
  const maxVisibleSlots = $derived.by(() => {
    if (containerWidth <= 0) return 1;
    return Math.max(1, Math.floor(containerWidth / MIN_PANE_WIDTH_PX));
  });

  const visibleSlots = $derived(slots.slice(0, maxVisibleSlots));

  /** Count of slots hidden behind the (future) overflow popover. Phase 4-G
   *  will surface these; Phase 4-B exposes the count via a
   *  `data-overflow-count` attribute on the root element so tests (and the
   *  Phase 4-G sub-agent) can read it without lifting state further. */
  const overflowCount = $derived(Math.max(0, slots.length - visibleSlots.length));

  /** paneforge minSize is percent of the group, not pixels. Convert and
   *  clamp: with many slots the raw percent gets tiny (10 slots at 520 px
   *  would be 5.2% each), and with one slot the clamp prevents 100%. */
  const minSizePercent = $derived.by(() => {
    if (containerWidth <= 0) return 10;
    const raw = (MIN_PANE_WIDTH_PX / containerWidth) * 100;
    return Math.max(5, Math.min(50, raw));
  });

  /** Default pane size if no persisted sizes match. Even distribution across
   *  the visible slots. */
  const defaultSize = $derived(
    visibleSlots.length > 0 ? 100 / visibleSlots.length : 100,
  );

  // Call sessionSetVisible whenever the set of VISIBLE slot ids changes. We
  // re-derive a join-string so Svelte's dirty-tracking doesn't over-fire on
  // order-only changes (paneforge layouts should not trigger a backend
  // round-trip).
  //
  // Review fix (Phase 4 perf): forward only the ids of slots that are
  // actually mounted (`visibleSlots`), not every slot. Slots in overflow
  // aren't rendered, so forwarding their PTY bytes across IPC is pure
  // waste. Bringing an overflow slot into view re-runs this effect and
  // replay-backlog handles the catch-up, so no data is lost.
  const visibleKey = $derived(
    visibleSlots
      .map((s) => s.session_id)
      .slice()
      .sort((a, b) => a - b)
      .join(','),
  );
  $effect(() => {
    // Reading `visibleKey` registers the dep; the `untrack` below keeps the
    // one-id-per-slot read out of the tracking graph so we don't loop.
    visibleKey;
    const ids = untrack(() => visibleSlots.map((s) => s.session_id));
    sessionSetVisible({ sessionIds: ids }).catch(() => {});
  });

  // Measure container width. ResizeObserver is available in jsdom for newer
  // versions but tests mock it; we feature-detect so SSR/unknown envs don't
  // crash.
  let ro: ResizeObserver | null = null;
  onMount(() => {
    if (!containerEl) return;
    // Initial read
    containerWidth = containerEl.clientWidth;
    if (typeof ResizeObserver !== 'undefined') {
      ro = new ResizeObserver((entries) => {
        for (const entry of entries) {
          containerWidth = entry.contentRect.width;
        }
      });
      ro.observe(containerEl);
    }
  });

  onDestroy(() => {
    ro?.disconnect();
    ro = null;
  });

  function handleLayoutChange(sizes: number[]): void {
    onResize?.(sizes);
  }

  // --- P4-G: overflow popover state ---------------------------------------
  //
  // The popover surfaces slots that don't fit at the minimum pane width
  // (index ≥ maxVisibleSlots). Clicking a hidden slot swaps it with the
  // rightmost visible slot so the user can bring it into view without
  // permanently reordering (the swap persists via onReorderSlots).
  //
  // Pattern mirrors LayoutSwitcher.svelte: click-outside, Escape, roving
  // tabindex for ArrowUp/Down/Home/End. In-component (not its own file)
  // because the popover has exactly one caller.
  let overflowOpen = $state(false);
  let overflowTriggerEl: HTMLButtonElement | undefined = $state();
  let overflowPopoverEl: HTMLDivElement | undefined = $state();
  let overflowFocusedIndex = $state(-1);

  /** Slots not in `visibleSlots` — populated with project + session lookups
   *  so the popover can render the same visual identity bits PaneSlot does
   *  (colour dot + project/label + status). */
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
      const color = getProjectColor(project) ?? undefined;
      let statusLabel = '';
      let statusClass = '';
      if (session) {
        switch (session.status) {
          case 'working': statusLabel = 'Working'; break;
          case 'idle': statusLabel = 'Idle'; break;
          case 'needs_input': statusLabel = 'Needs Input'; break;
          case 'ended': statusLabel = 'Ended'; break;
          case 'error': statusLabel = 'Error'; break;
          default: statusLabel = session.status;
        }
        statusClass = `status-${session.status.replaceAll('_', '-')}`;
      }
      return {
        slot,
        absoluteIndex,
        projectName: project?.display_name ?? 'Project',
        sessionLabel: session?.label ?? `Session ${slot.session_id}`,
        statusLabel,
        statusClass,
        color,
      };
    });
  });

  function toggleOverflow(): void {
    if (overflowOpen) {
      closeOverflow();
    } else {
      overflowOpen = true;
      overflowFocusedIndex = -1;
    }
  }

  function closeOverflow(): void {
    overflowOpen = false;
    overflowFocusedIndex = -1;
    overflowTriggerEl?.focus();
  }

  /** Swap the chosen overflow slot with the rightmost visible slot so it
   *  becomes visible. Persists via onReorderSlots. */
  function bringIntoView(absoluteIndex: number): void {
    if (!onReorderSlots) return;
    const rightmost = maxVisibleSlots - 1;
    if (absoluteIndex <= rightmost || absoluteIndex >= slots.length) return;
    const next = slots.slice();
    [next[rightmost], next[absoluteIndex]] = [next[absoluteIndex], next[rightmost]];
    onReorderSlots(next.map((s, i) => ({ ...s, order: i })));
    closeOverflow();
  }

  function handleOverflowDocumentClick(event: MouseEvent): void {
    if (!overflowOpen) return;
    const target = event.target as Node;
    if (overflowPopoverEl?.contains(target)) return;
    if (overflowTriggerEl?.contains(target)) return;
    overflowOpen = false;
    overflowFocusedIndex = -1;
  }

  function handleOverflowKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeOverflow();
      return;
    }
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
    const items = overflowPopoverEl.querySelectorAll<HTMLButtonElement>(
      '.pane-overflow-item',
    );
    items[overflowFocusedIndex]?.focus();
  }

  // When the popover first opens, wait a tick for the DOM nodes to exist
  // before attempting to focus the first item on ArrowDown. We don't
  // auto-focus on open (matches LayoutSwitcher) — the trigger button's
  // invoker keeps focus until they press ArrowDown / ArrowUp.
  $effect(() => {
    if (!overflowOpen) return;
    // If overflow disappears while open (resize widens container), close.
    if (overflowCount === 0) {
      untrack(() => {
        overflowOpen = false;
        overflowFocusedIndex = -1;
      });
    }
  });

  onMount(() => {
    document.addEventListener('click', handleOverflowDocumentClick, true);
  });

  onDestroy(() => {
    document.removeEventListener('click', handleOverflowDocumentClick, true);
  });

  /** Stable key so panes keep identity when reordered or clipped by overflow. */
  function paneKey(slot: PaneSlotType): string {
    return String(slot.session_id);
  }
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
    <PaneGroup
      direction="horizontal"
      class="pane-workspace-group"
      onLayoutChange={handleLayoutChange}
    >
      {#each visibleSlots as slot, i (paneKey(slot))}
        <Pane
          order={slot.order}
          minSize={minSizePercent}
          defaultSize={paneSizes && paneSizes[i] !== undefined && paneSizes.length === visibleSlots.length
            ? paneSizes[i]
            : defaultSize}
          class="pane-workspace-pane"
        >
          <PaneSlot
            sessionId={slot.session_id}
            highlighted={highlightedSessionId === slot.session_id}
            {highlightToken}
            onClose={() => onSlotClose(slot.session_id)}
            onFocus={() => onSlotFocus(slot.session_id)}
            onMoveLeft={onReorderSlots && slots.length > 1 ? () => handleMoveLeft(slot.session_id) : undefined}
            onMoveRight={onReorderSlots && slots.length > 1 ? () => handleMoveRight(slot.session_id) : undefined}
            canMoveLeft={i > 0}
            canMoveRight={i < slots.length - 1}
            onReorderDragStart={onReorderSlots && slots.length > 1
              ? (e) => handleReorderDragStart(slot.session_id, e)
              : undefined}
            onReorderDrop={onReorderSlots && slots.length > 1
              ? () => handleReorderDrop(slot.session_id)
              : undefined}
            onReorderDragOver={onReorderSlots && slots.length > 1
              ? () => {}
              : undefined}
          />
        </Pane>
        {#if i < visibleSlots.length - 1}
          <PaneResizer class="pane-workspace-resizer" />
        {/if}
      {/each}
    </PaneGroup>
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

  .pane-workspace :global(.pane-workspace-group) {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    width: 100%;
    height: 100%;
  }

  .pane-workspace :global(.pane-workspace-pane) {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  /* Visible resizer gutter between panes. 4 px clickable target, 1 px
     visible line centered with background-clip. Matches the divider
     pattern SplitView uses internally. */
  .pane-workspace :global(.pane-workspace-resizer) {
    flex: 0 0 4px;
    background: var(--color-border, #2a2d35);
    cursor: col-resize;
    position: relative;
    transition: background 150ms;
  }

  .pane-workspace :global(.pane-workspace-resizer:hover),
  .pane-workspace :global(.pane-workspace-resizer[data-active]) {
    background: var(--color-accent, #60a5fa);
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

  /* P4-G: overflow indicator + popover. The trigger sits absolutely at the
     top-right of the pane workspace; the popover drops down beneath it.
     Follows the LayoutSwitcher pattern for visual weight so it reads as a
     peer control in the header strip. */
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

  /* Badge base + status colours — duplicated from PaneSlot so the popover
     reads consistently without depending on another component's styles
     being loaded first. */
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

  .status-working {
    background: var(--color-success-bg, #064e3b);
    color: var(--color-success, #34d399);
  }

  .status-idle {
    background: var(--color-info-bg, #1e3a5f);
    color: var(--color-info, #60a5fa);
  }

  .status-needs-input {
    background: var(--color-warning-bg, #3d2e00);
    color: var(--color-warning, #fbbf24);
  }

  .status-ended {
    background: var(--color-muted-bg, #1e2028);
    color: var(--color-text-muted, #8b8fa3);
  }

  .status-error {
    background: var(--color-error-bg, #4c0519);
    color: var(--color-error, #f87171);
  }
</style>
