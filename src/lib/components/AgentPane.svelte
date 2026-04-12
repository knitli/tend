<!--
  T096: AgentPane — xterm.js instance for agent PTY output.

  Input path is gated on isInteractive:
  - workbench-owned + not reattached_mirror → interactive (keystrokes dispatched)
  - wrapper-owned → read-only with "Read-only mirror" banner
  - reattached-mirror → read-only with "Read-only after restart" banner
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { createTerminal, type CreatedTerminal } from '$lib/xterm/createTerminal';
  import { sessionSendInput, sessionResize, onSessionOutput } from '$lib/api/sessions';
  import type { SessionSummary } from '$lib/api/sessions';
  import type { UnlistenFn } from '@tauri-apps/api/event';

  interface Props {
    session: SessionSummary;
  }

  let { session }: Props = $props();

  let containerEl: HTMLDivElement | undefined = $state();
  let created: CreatedTerminal | undefined = $state();

  const isInteractive = $derived(
    session.ownership === 'workbench' && !session.reattached_mirror,
  );

  const readOnlyMessage = $derived.by(() => {
    if (session.ownership === 'wrapper') {
      return 'Read-only mirror — type in the launching terminal';
    }
    if (session.reattached_mirror) {
      return 'Read-only after workbench restart — end and respawn to regain input';
    }
    return null;
  });

  let unlisten: UnlistenFn | undefined;
  // L4 fix: explicitly track xterm disposables.
  let disposables: { dispose(): void }[] = [];

  onMount(async () => {
    if (!containerEl) return;

    created = createTerminal(containerEl, {
      cursorBlink: isInteractive,
      disableStdin: !isInteractive,
    });

    // Subscribe to PTY output for this session.
    unlisten = await onSessionOutput((payload) => {
      if (payload.session_id !== session.id) return;
      // Decode base64 bytes.
      const decoded = atob(payload.bytes);
      const bytes = new Uint8Array(decoded.length);
      for (let i = 0; i < decoded.length; i++) {
        bytes[i] = decoded.charCodeAt(i);
      }
      created?.terminal.write(bytes);
    });

    // Wire keystrokes for interactive sessions.
    if (isInteractive) {
      disposables.push(
        created.terminal.onData((data) => {
          sessionSendInput({ sessionId: session.id, bytes: data }).catch(() => {});
        }),
      );
    }

    // Wire resize events.
    disposables.push(
      created.terminal.onResize(({ cols, rows }) => {
        if (isInteractive) {
          sessionResize({ sessionId: session.id, cols, rows }).catch(() => {});
        }
      }),
    );
  });

  onDestroy(() => {
    unlisten?.();
    for (const d of disposables) d.dispose();
    created?.dispose();
  });
</script>

<div class="agent-pane">
  {#if readOnlyMessage}
    <div class="readonly-banner" role="status">
      {readOnlyMessage}
    </div>
  {/if}
  <div class="terminal-container" bind:this={containerEl} role="application" aria-label="Agent terminal output"></div>
</div>

<style>
  .agent-pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .readonly-banner {
    padding: 4px 12px;
    background: var(--color-warning-bg, #713f12);
    color: var(--color-warning, #fbbf24);
    font-size: 0.6875rem;
    font-weight: 600;
    text-align: center;
    letter-spacing: 0.02em;
    flex-shrink: 0;
  }

  .terminal-container {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
