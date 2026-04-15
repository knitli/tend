<!--
  PaneSlot — one slot in the horizontal pane workspace (Phase 4-C).

  Wraps the existing SplitView with a compact per-pane header that carries:
  - project colour dot (see `getProjectColor`)
  - project name · session label
  - status badge (reusing SessionRow's status-* classes)
  - focus (⊙) / close (×) buttons
  - drag handle (⠿) rendered inert in Phase 4-B/C; Phase 4-D attaches DnD to
    the `[data-drag-handle]` element.

  The header tint mirrors `.active-session-header` from +page.svelte (4px
  left strip + 8% colour-mix bg) but at a tighter padding because multiple
  headers will stack side-by-side.

  If the session id is missing from `sessionsStore` (race on delete or
  hydrate-time filter missed it), a tiny "session not found" placeholder is
  rendered instead — only a × button, to avoid crashing the whole workspace.
-->
<script lang="ts">
  import SplitView from '$lib/components/SplitView.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { getProjectColor } from '$lib/util/projectColor';

  interface Props {
    sessionId: number;
    onFocus: () => void;
    onClose: () => void;
    /** When true, this slot matches the most-recent activation and should
     *  flash (Phase 1). Pair with `highlightToken` so repeat activations
     *  restart the animation even when `highlighted` stays true. */
    highlighted?: boolean;
    highlightToken?: number;
    /** P4-D: keyboard-accessible slot reorder. When provided, the drag-handle
     *  area reveals `‹` / `›` buttons that shift this pane one slot left or
     *  right. Either may be `undefined` (for the leftmost / rightmost slot);
     *  the button is then disabled. This is the non-drag path — native HTML5
     *  drag on the `data-drag-handle` element fires `onReorderDragStart` /
     *  `onReorderDragOver` / `onReorderDrop` which the parent coordinates. */
    onMoveLeft?: () => void;
    onMoveRight?: () => void;
    canMoveLeft?: boolean;
    canMoveRight?: boolean;
    /** P4-D: native drag wiring for pane-slot reorder. The parent
     *  (PaneWorkspace) provides these so the handle initiates a drag that
     *  peer slots accept as `application/x-tend-pane-slot`. */
    onReorderDragStart?: (event: DragEvent) => void;
    /** Called (no args) while the drag hovers this slot; PaneWorkspace's
     *  closure already knows which slot is targeted, so the event is not
     *  forwarded. Simplified from the original `(event: DragEvent) => void`
     *  so PaneWorkspace can pass a no-arg closure without TS complaints. */
    onReorderDragOver?: () => void;
    /** Called (no args) when the drag is dropped onto this slot. */
    onReorderDrop?: () => void;
    /** Called when dragend fires on the handle — covers both a successful
     *  drop and a cancelled drag (e.g. Escape / drop outside any target).
     *  PaneWorkspace uses this to clear its reorderDragSessionId state so
     *  a stale id can't interfere with the next drag. */
    onReorderDragEnd?: () => void;
    /** Called when the drag cursor leaves this slot's bounds. PaneWorkspace
     *  uses this to clear the drop-indicator for this slot. */
    onReorderDragLeave?: () => void;
    /** True while this slot's pane is the one being dragged — dims it
     *  visually to communicate "source slot". */
    isBeingDragged?: boolean;
    /** Insertion indicator driven by PaneWorkspace: which edge of this slot
     *  will receive the dragged pane.
     *  - 'before' → left-edge bar (dragged pane lands before this slot)
     *  - 'after'  → right-edge bar (dragged pane lands after this slot)
     *  - null     → no indicator */
    dropIndicator?: 'before' | 'after' | null;
  }

  let {
    sessionId,
    onFocus,
    onClose,
    highlighted = false,
    highlightToken = 0,
    onMoveLeft,
    onMoveRight,
    canMoveLeft = false,
    canMoveRight = false,
    onReorderDragStart,
    onReorderDragOver,
    onReorderDrop,
    onReorderDragEnd,
    onReorderDragLeave,
    isBeingDragged = false,
    dropIndicator = null,
  }: Props = $props();

  function handleDragOver(event: DragEvent): void {
    if (!onReorderDragOver) return;
    // Only accept pane-slot drags (not session-source). We rely on the
    // dataTransfer types check rather than the payload (which is
    // unreadable during dragover in most browsers).
    if (!event.dataTransfer) return;
    if (!event.dataTransfer.types.includes('application/x-tend-pane-slot')) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    onReorderDragOver();
  }

  function handleDragLeave(event: DragEvent): void {
    // Guard: dragleave fires when the cursor enters a child element of this
    // slot. Only forward the event when the cursor truly left our bounds
    // (relatedTarget is null or outside this element).
    const rel = event.relatedTarget as Node | null;
    if (rel && (event.currentTarget as HTMLElement)?.contains(rel)) return;
    onReorderDragLeave?.();
  }

  function handleDrop(event: DragEvent): void {
    if (!onReorderDrop) return;
    if (!event.dataTransfer?.types.includes('application/x-tend-pane-slot')) return;
    event.preventDefault();
    onReorderDrop();
  }

  const session = $derived(sessionsStore.byId(sessionId) ?? null);
  const project = $derived(session ? projectsStore.byId(session.project_id) ?? null : null);
  const projectColor = $derived(getProjectColor(project));

  const statusLabel = $derived.by(() => {
    if (!session) return '';
    switch (session.status) {
      case 'working':
        return 'Working';
      case 'idle':
        return 'Idle';
      case 'needs_input':
        return 'Needs Input';
      case 'ended':
        return 'Ended';
      case 'error':
        return 'Error';
      default:
        return session.status;
    }
  });

  const statusClass = $derived(
    session ? `status-${session.status.replaceAll('_', '-')}` : '',
  );

  /** Only the currently-active pane should animate. `SplitView` re-runs its
   *  flash effect whenever the token changes, so non-highlighted panes pass
   *  0 to keep it dormant. */
  const effectiveToken = $derived(highlighted ? highlightToken : 0);

  const isReadOnly = $derived(
    session ? session.ownership === 'wrapper' || session.reattached_mirror : false,
  );
</script>

<div
  class="pane-slot"
  class:highlighted
  class:being-dragged={isBeingDragged}
  class:drop-before={dropIndicator === 'before'}
  class:drop-after={dropIndicator === 'after'}
  style={projectColor ? `--project-color: ${projectColor}` : ''}
  data-session-id={sessionId}
  role="region"
  aria-label={`Pane for session ${sessionId}`}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  {#if session}
    <header class="pane-slot-header">
      <!-- P4-D: drag handle for slot reorder. Native HTML5 drag — starts a
           drag with a custom dataTransfer type so peer slots recognise it
           and the SessionList's session-source dnd zone ignores it. Also
           renders inline `‹` / `›` move buttons for keyboard-accessible
           reorder (the non-drag path). -->
      <span
        class="pane-slot-drag-handle"
        data-drag-handle
        draggable={onReorderDragStart ? 'true' : 'false'}
        ondragstart={onReorderDragStart}
        ondragend={onReorderDragEnd}
        aria-hidden="true"
        title={onReorderDragStart ? 'Drag to reorder pane' : 'Only one pane open'}
      >⠿</span>
      {#if onMoveLeft}
        <button
          type="button"
          class="pane-slot-btn pane-slot-move-btn"
          onclick={onMoveLeft}
          disabled={!canMoveLeft}
          title="Move pane left"
          aria-label="Move pane left"
        >
          ‹
        </button>
      {/if}
      {#if onMoveRight}
        <button
          type="button"
          class="pane-slot-btn pane-slot-move-btn"
          onclick={onMoveRight}
          disabled={!canMoveRight}
          title="Move pane right"
          aria-label="Move pane right"
        >
          ›
        </button>
      {/if}
      <span class="pane-slot-dot" aria-hidden="true"></span>
      <span class="pane-slot-title" title={`${project?.display_name ?? 'Project'} · ${session.label}`}>
        <strong class="pane-slot-project">{project?.display_name ?? 'Project'}</strong>
        <span class="pane-slot-separator" aria-hidden="true">·</span>
        <span class="pane-slot-label">{session.label}</span>
      </span>
      <span class="badge {statusClass}">{statusLabel}</span>
      {#if isReadOnly}
        <span class="readonly-banner" title="Read-only mirror">Read-only</span>
      {/if}
      <button
        type="button"
        class="pane-slot-btn pane-slot-focus-btn"
        onclick={onFocus}
        title="Focus on this session"
        aria-label="Focus on this session"
      >
        ⊙
      </button>
      <button
        type="button"
        class="pane-slot-btn pane-slot-close-btn"
        onclick={onClose}
        title="Close this pane"
        aria-label="Close this pane"
      >
        ×
      </button>
    </header>
    <div class="pane-slot-body">
      <SplitView sessionId={session.id} {session} highlightToken={effectiveToken} />
    </div>
  {:else}
    <header class="pane-slot-header pane-slot-header-missing">
      <span class="pane-slot-title">
        <strong>Session not found</strong>
      </span>
      <button
        type="button"
        class="pane-slot-btn pane-slot-close-btn"
        onclick={onClose}
        title="Close this pane"
        aria-label="Close this pane"
      >
        ×
      </button>
    </header>
    <div class="pane-slot-body pane-slot-missing-body">
      <p class="muted">This session is no longer available.</p>
    </div>
  {/if}
</div>

<style>
  .pane-slot {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
    position: relative; /* required for ::before/::after insertion indicators */
  }

  /* P4-D: source slot dimming while its handle is being dragged.
     The dimmed look communicates "this pane is in motion" without
     visually removing it (so the user can still see the original layout). */
  .pane-slot.being-dragged {
    opacity: 0.4;
    transition: opacity 120ms;
  }

  /* P4-D: left-edge insertion indicator — "the dragged pane will be placed
     BEFORE this slot." 3 px accent bar runs full-height on the left edge.
     Implemented with ::before so it overlays the project-colour strip
     without shifting any layout. */
  .pane-slot.drop-before::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--color-accent, #60a5fa);
    z-index: 2;
    pointer-events: none;
    border-radius: 2px 0 0 2px;
  }

  /* P4-D: right-edge insertion indicator — "the dragged pane will be placed
     AFTER this slot." */
  .pane-slot.drop-after::after {
    content: '';
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--color-accent, #60a5fa);
    z-index: 2;
    pointer-events: none;
    border-radius: 0 2px 2px 0;
  }

  .pane-slot-header {
    display: flex;
    align-items: center;
    gap: var(--space-2, 0.5rem);
    padding: 0.375rem 0.5rem;
    border-bottom: 1px solid var(--color-border, #2a2d35);
    /* Mirrors .active-session-header in +page.svelte: 4 px project-colour
       left strip + 8% tinted bg. Compacted for multi-pane stacking. */
    border-left: 4px solid var(--project-color, var(--color-accent, #60a5fa));
    background: color-mix(
      in srgb,
      var(--project-color, var(--color-accent, #60a5fa)) 8%,
      var(--color-surface, #0f1115)
    );
    flex-shrink: 0;
    min-width: 0;
  }

  .pane-slot-drag-handle {
    cursor: grab;
    color: var(--color-text-muted, #8b8fa3);
    opacity: 0.45;
    font-size: 0.875rem;
    line-height: 1;
    user-select: none;
    padding: 0 2px;
  }

  .pane-slot-drag-handle:hover {
    opacity: 0.9;
  }

  .pane-slot-drag-handle[draggable="true"]:active {
    cursor: grabbing;
  }

  .pane-slot-move-btn {
    width: 18px;
    height: 18px;
    font-size: 0.8125rem;
    padding: 0;
  }

  .pane-slot-move-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .pane-slot-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    flex-shrink: 0;
  }

  .pane-slot-title {
    display: inline-flex;
    align-items: baseline;
    gap: 0.375rem;
    min-width: 0;
    flex: 1;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 0.8125rem;
    color: var(--color-text, #e6e8ef);
  }

  .pane-slot-project {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }

  .pane-slot-separator {
    color: var(--color-text-muted, #8b8fa3);
    opacity: 0.6;
    flex-shrink: 0;
  }

  .pane-slot-label {
    color: var(--color-text-muted, #8b8fa3);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }

  /* Shared badge base — duplicated from SessionRow so PaneSlot doesn't
     depend on it loading somewhere in the tree first. */
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

  .readonly-banner {
    padding: 1px 6px;
    border-radius: var(--radius-sm, 4px);
    background: var(--color-warning-bg, #3d2e00);
    color: var(--color-warning, #fbbf24);
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    flex-shrink: 0;
  }

  .pane-slot-btn {
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.9375rem;
    line-height: 1;
    cursor: pointer;
    transition: background 150ms, color 150ms, opacity 150ms;
    flex-shrink: 0;
  }

  .pane-slot-btn:hover,
  .pane-slot-btn:focus-visible {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .pane-slot-focus-btn {
    font-size: 0.875rem;
  }

  .pane-slot-close-btn {
    font-size: 1rem;
  }

  .pane-slot-body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .pane-slot-missing-body {
    align-items: center;
    justify-content: center;
    padding: var(--space-4, 1rem);
  }

  .pane-slot-header-missing {
    border-left-color: var(--color-border, #2a2d35);
    background: var(--color-surface, #0f1115);
  }

  .muted {
    margin: 0;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.875rem;
  }
</style>
