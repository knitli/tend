<!--
  T097: CompanionPane — xterm.js instance for the companion shell.

  Subscribes to companion:output for the active session, dispatches
  keystrokes to companionSendInput, handles companion_resize on container
  resize via FitAddon.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { createTerminal, type CreatedTerminal } from '$lib/xterm/createTerminal';
  import { companionSendInput, companionResize, companionRespawn, onCompanionOutput } from '$lib/api/companions';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  interface Props {
    sessionId: number;
  }

  let { sessionId }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let created: CreatedTerminal | undefined = $state();
  let unlisten: UnlistenFn | undefined;
  let disposables: { dispose(): void }[] = [];

  onMount(async () => {
    if (!containerEl) return;

    created = createTerminal(containerEl);

    // Subscribe to companion output.
    unlisten = await onCompanionOutput((payload) => {
      if (payload.session_id !== sessionId) return;
      const decoded = atob(payload.bytes);
      const bytes = new Uint8Array(decoded.length);
      for (let i = 0; i < decoded.length; i++) {
        bytes[i] = decoded.charCodeAt(i);
      }
      created?.terminal.write(bytes);
    });

    // Wire keystrokes (L4: track disposable).
    disposables.push(
      created.terminal.onData((data) => {
        companionSendInput({ sessionId, bytes: data }).catch(() => {});
      }),
    );

    // Wire resize events.
    disposables.push(
      created.terminal.onResize(({ cols, rows }) => {
        companionResize({ sessionId, cols, rows }).catch(() => {});
      }),
    );
  });

  onDestroy(() => {
    unlisten?.();
    for (const d of disposables) d.dispose();
    created?.dispose();
  });

  async function handleRespawn() {
    try {
      await companionRespawn({ sessionId });
      // H8 fix: full reset clears both scrollback and viewport for clean slate.
      created?.terminal.reset();
    } catch (err: unknown) {
      // M8 fix: surface respawn errors in the terminal instead of silently ignoring.
      const msg = err instanceof Error ? err.message : String(err);
      created?.terminal.writeln(`\r\n\x1b[31m[Respawn failed: ${msg}]\x1b[0m`);
    }
  }
</script>

<div class="companion-pane">
  <div class="companion-header">
    <span class="companion-label">Companion Shell</span>
    <button
      class="respawn-btn"
      onclick={handleRespawn}
      title="Restart companion shell"
    >
      Restart
    </button>
  </div>
  <div class="terminal-container" bind:this={containerEl} role="application" aria-label="Companion shell"></div>
</div>

<style>
  .companion-pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .companion-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 12px;
    background: var(--color-surface-raised, #15171c);
    border-bottom: 1px solid var(--color-border, #2a2d35);
    flex-shrink: 0;
  }

  .companion-label {
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #8b8fa3);
  }

  .respawn-btn {
    padding: 2px 8px;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 4px;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.6875rem;
    transition: background-color 150ms, color 150ms;
  }

  .respawn-btn:hover {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
  }

  .terminal-container {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    /* Horizontal breathing room only — companion header already provides
       top separation. Padding here is safe because we wrap xterm in an
       inner div only when used from AgentPane; the companion's xterm fills
       directly, so keep padding off .terminal-container to avoid clipping. */
  }

  .companion-pane {
    padding: 0 6px 0 6px;
  }
</style>
