<!--
  AddSlotZone — a thin vertical drop strip rendered at the right edge of the
  pane workspace (Phase 4-D). Accepts items of type `'session-source'` from
  SessionList and calls `onDrop(sessionId)` when a session is dropped in.

  Visual:
    - idle state: dashed vertical border, low-opacity placeholder glyph
    - dragging state (hovered by a compatible item): solid accent border +
      "drop here" label

  svelte-dnd-action quirks worked around here:
    - The zone's `items` array is synced from the consider event (which
      inserts a shadow placeholder while the drag is in progress). On
      finalize, we extract the "real" dropped item (the one that isn't
      the shadow placeholder) and forward its sessionId to the parent.
    - We never persist the items array — this zone's only purpose is to
      capture the drop event. The items always restore to empty after a
      drop so the next drag starts clean.
-->
<script lang="ts">
  import {
    dndzone,
    SHADOW_PLACEHOLDER_ITEM_ID,
    type DndEvent,
  } from 'svelte-dnd-action';

  interface Props {
    /** Called with the session id that was dropped onto this zone. */
    onDrop: (sessionId: number) => void;
  }

  let { onDrop }: Props = $props();

  type DnDSessionItem = { id: string; sessionId: number };

  /** Items currently "in" this zone. Always empty canonically — we only
   *  receive transient items during a drag from SessionList. After the
   *  drop we reset to []. */
  let items = $state<DnDSessionItem[]>([]);
  let hovering = $state(false);

  function handleConsider(e: CustomEvent<DndEvent<DnDSessionItem>>): void {
    items = e.detail.items;
    hovering = items.length > 0;
  }

  function handleFinalize(e: CustomEvent<DndEvent<DnDSessionItem>>): void {
    // Filter out svelte-dnd-action's shadow placeholder (leftover if any)
    // and call onDrop for every "real" dropped item. In practice there's
    // always exactly one; the loop guards against future multi-drag.
    for (const item of e.detail.items) {
      if (item.id !== SHADOW_PLACEHOLDER_ITEM_ID && typeof item.sessionId === 'number') {
        onDrop(item.sessionId);
      }
    }
    items = [];
    hovering = false;
  }
</script>

<div
  class="add-slot-zone"
  class:hovering
  use:dndzone={{
    items,
    type: 'session-source',
    flipDurationMs: 0,
    morphDisabled: true,
    dropTargetStyle: {},
  }}
  onconsider={handleConsider}
  onfinalize={handleFinalize}
  aria-label="Drop zone: add session to a new pane"
>
  {#if hovering}
    <span class="label">Drop to open</span>
  {:else}
    <span class="placeholder" aria-hidden="true">+</span>
  {/if}
  {#each items as item (item.id)}
    <!-- svelte-dnd-action needs direct children matching the items array.
         We hide the shadow placeholder visually so it doesn't stretch
         the zone's layout. -->
    <div class="dnd-shadow" data-item-id={item.id}></div>
  {/each}
</div>

<style>
  .add-slot-zone {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 40px;
    min-width: 40px;
    height: 100%;
    border-left: 1px dashed var(--color-border, #2a2d35);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.75rem;
    opacity: 0.55;
    transition: background 150ms, border-color 150ms, opacity 150ms, color 150ms;
    position: relative;
    writing-mode: vertical-rl;
    text-orientation: mixed;
    padding: var(--space-2, 0.5rem) 0;
    user-select: none;
  }

  .add-slot-zone:hover {
    opacity: 0.85;
  }

  .add-slot-zone.hovering {
    border-left-style: solid;
    border-left-color: var(--color-accent, #60a5fa);
    background: color-mix(
      in srgb,
      var(--color-accent, #60a5fa) 12%,
      var(--color-surface, #0f1115)
    );
    color: var(--color-accent, #60a5fa);
    opacity: 1;
  }

  .placeholder {
    font-size: 1rem;
    font-weight: 600;
    writing-mode: horizontal-tb;
  }

  .label {
    writing-mode: vertical-rl;
    text-orientation: mixed;
    letter-spacing: 0.05em;
    font-weight: 500;
  }

  .dnd-shadow {
    display: none;
  }
</style>
