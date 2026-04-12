<!--
  T098: SplitView — horizontal split with AgentPane (left) + CompanionPane (right).

  Calls sessionActivate on mount to fetch the session + companion,
  has a draggable divider, cleans up on unmount.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import AgentPane from '$lib/components/AgentPane.svelte';
  import CompanionPane from '$lib/components/CompanionPane.svelte';
  import Scratchpad from '$lib/components/Scratchpad.svelte';
  import { sessionActivate, type Session, type SessionSummary } from '$lib/api/sessions';
  import type { CompanionTerminal } from '$lib/api/companions';
  import { workspaceStore } from '$lib/stores/workspace.svelte';

  interface Props {
    sessionId: number;
    /** Full session summary from the store — used for display. */
    session: SessionSummary;
  }

  let { sessionId, session }: Props = $props();

  let companion = $state<CompanionTerminal | null>(null);
  let activating = $state(true);
  let error = $state<string | null>(null);

  // Divider drag state.
  let splitPercent = $state(50);
  let dragging = $state(false);
  let containerEl: HTMLDivElement | undefined = $state();

  onMount(async () => {
    try {
      const result = await sessionActivate({ sessionId });
      companion = result.companion;
      error = null;
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      error = msg;
    } finally {
      activating = false;
    }
  });

  // T117: Scratchpad toggle. L3: persist via workspace ui map.
  let scratchpadVisible = $state(
    workspaceStore.current.ui?.scratchpad_visible === true
  );

  function toggleScratchpad(): void {
    scratchpadVisible = !scratchpadVisible;
    workspaceStore.setUi('scratchpad_visible', scratchpadVisible);
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    // Ctrl+Shift+S toggles the scratchpad.
    if (e.key === 'S' && e.ctrlKey && e.shiftKey) {
      e.preventDefault();
      toggleScratchpad();
    }
  }

  // H7 fix: track active drag listeners for cleanup on unmount.
  let dragCleanup: (() => void) | null = null;

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    dragging = true;

    function handleMouseMove(e: MouseEvent) {
      if (!containerEl) return;
      const rect = containerEl.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const pct = Math.max(20, Math.min(80, (x / rect.width) * 100));
      splitPercent = pct;
    }

    function handleMouseUp() {
      dragging = false;
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
      dragCleanup = null;
    }

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    dragCleanup = () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }

  onDestroy(() => {
    dragCleanup?.();
  });

  // M6: keyboard accessibility for the divider.
  const SPLIT_MIN = 20;
  const SPLIT_MAX = 80;
  const SPLIT_STEP = 5;

  function handleDividerKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') {
      e.preventDefault();
      splitPercent = Math.max(SPLIT_MIN, splitPercent - SPLIT_STEP);
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowUp') {
      e.preventDefault();
      splitPercent = Math.min(SPLIT_MAX, splitPercent + SPLIT_STEP);
    } else if (e.key === 'Home') {
      e.preventDefault();
      splitPercent = SPLIT_MIN;
    } else if (e.key === 'End') {
      e.preventDefault();
      splitPercent = SPLIT_MAX;
    }
  }
</script>

<div class="split-view" bind:this={containerEl} class:dragging>
  {#if activating}
    <div class="loading">
      <p class="muted">Activating session...</p>
    </div>
  {:else if error}
    <div class="error-state">
      <p class="muted">Failed to activate: {error}</p>
    </div>
  {:else}
    <div class="left-pane" style="flex: {splitPercent} 0 0%">
      <AgentPane {session} />
    </div>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions a11y_no_noninteractive_tabindex -->
    <div
      class="divider"
      role="separator"
      aria-orientation="vertical"
      aria-valuenow={Math.round(splitPercent)}
      aria-valuemin={SPLIT_MIN}
      aria-valuemax={SPLIT_MAX}
      aria-label="Resize panes"
      tabindex="0"
      onmousedown={handleMouseDown}
      onkeydown={handleDividerKeydown}
    ></div>

    <div class="right-pane" style="flex: {100 - splitPercent} 0 0%">
      {#if companion}
        <CompanionPane {sessionId} />
      {:else}
        <div class="no-companion">
          <p class="muted">No companion terminal available.</p>
        </div>
      {/if}
    </div>

    {#if scratchpadVisible}
      <div class="scratchpad-panel">
        <div class="scratchpad-toolbar">
          <span class="scratchpad-title">Scratchpad</span>
          <button class="close-btn" onclick={toggleScratchpad} title="Close scratchpad (Ctrl+Shift+S)">
            Close
          </button>
        </div>
        <Scratchpad projectId={session.project_id} />
      </div>
    {/if}
  {/if}
</div>

<svelte:window onkeydown={handleGlobalKeydown} />

<style>
  .split-view {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .split-view.dragging {
    cursor: col-resize;
    user-select: none;
  }

  .left-pane,
  .right-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .divider {
    width: 4px;
    flex-shrink: 0;
    background: var(--color-border, #2a2d35);
    cursor: col-resize;
    transition: background-color 150ms;
  }

  .divider:hover,
  .split-view.dragging .divider {
    background: var(--color-accent, #60a5fa);
  }

  .loading,
  .error-state,
  .no-companion {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
  }

  .scratchpad-panel {
    width: 300px;
    min-width: 200px;
    border-left: 1px solid var(--color-border, #2a2d35);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    overflow: hidden;
  }

  .scratchpad-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.375rem 0.5rem;
    border-bottom: 1px solid var(--color-border, #2a2d35);
    flex-shrink: 0;
  }

  .scratchpad-title {
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #8b8fa3);
  }

  .close-btn {
    padding: 2px 8px;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.6875rem;
  }

  .close-btn:hover {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
  }

  .muted {
    margin: 0;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.875rem;
  }
</style>
