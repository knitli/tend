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
  import { sessionSetVisible } from '$lib/api/sessions';
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
  }

  let {
    slots,
    paneSizes,
    highlightedSessionId = null,
    highlightToken = 0,
    onSlotClose,
    onSlotFocus,
    onResize,
  }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let containerWidth = $state(0);

  /** How many slots comfortably fit at MIN_PANE_WIDTH_PX. Always at least 1
   *  (even in a window so narrow that `floor(w / 520) == 0` we still show
   *  one pane rather than an empty workspace). */
  const maxVisibleSlots = $derived.by(() => {
    if (containerWidth <= 0) return Math.max(1, slots.length);
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

  // Call sessionSetVisible whenever the slot id set changes. We re-derive a
  // join-string so Svelte's dirty-tracking doesn't over-fire on order-only
  // changes (paneforge layouts should not trigger a backend round-trip).
  const visibleKey = $derived(slots.map((s) => s.session_id).join(','));
  $effect(() => {
    // Track the key so the effect re-runs on changes.
    const _ = visibleKey;
    const ids = untrack(() => slots.map((s) => s.session_id));
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

  /** paneforge keys panes by `order` to preserve identity when the list
   *  mutates. We use session_id as the stable key plus index-based order. */
  function paneKey(slot: PaneSlotType, index: number): string {
    return `${slot.session_id}-${index}`;
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
  {:else}
    <PaneGroup
      direction="horizontal"
      class="pane-workspace-group"
      onLayoutChange={handleLayoutChange}
    >
      {#each visibleSlots as slot, i (paneKey(slot, i))}
        <Pane
          order={i}
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
          />
        </Pane>
        {#if i < visibleSlots.length - 1}
          <PaneResizer class="pane-workspace-resizer" />
        {/if}
      {/each}
    </PaneGroup>
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
</style>
