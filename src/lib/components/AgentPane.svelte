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
  import {
    sessionSendInput,
    sessionResize,
    sessionReadBacklog,
    onSessionOutput,
  } from '$lib/api/sessions';
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

  function decodeBase64(b64: string): Uint8Array {
    const decoded = atob(b64);
    const bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) {
      bytes[i] = decoded.charCodeAt(i);
    }
    return bytes;
  }

  onMount(async () => {
    if (!containerEl) return;

    created = createTerminal(containerEl, {
      cursorBlink: isInteractive,
      disableStdin: !isInteractive,
    });
    const terminal = created.terminal;

    // Two-phase setup to handle the race between the supervisor starting
    // to emit PTY bytes and this component's listener being ready:
    //
    //   1. Subscribe first — buffer live events into a queue.
    //   2. Fetch the backend replay backlog (bytes emitted before the
    //      listener was registered) and write them to xterm.
    //   3. Flush the queued live events.
    //   4. Switch the handler to write directly to xterm.
    //
    // For TUIs like Claude there may be a small overlap between the tail
    // of the backlog and the head of the live queue; duplicate ANSI
    // sequences replayed twice are a no-op for the terminal.
    const liveQueue: Uint8Array[] = [];
    let replayed = false;

    unlisten = await onSessionOutput((payload) => {
      if (payload.session_id !== session.id) return;
      const bytes = decodeBase64(payload.bytes);
      if (replayed) {
        terminal.write(bytes);
      } else {
        liveQueue.push(bytes);
      }
    });

    try {
      const { bytes } = await sessionReadBacklog({ sessionId: session.id });
      if (bytes) {
        terminal.write(decodeBase64(bytes));
      }
    } catch {
      // Backlog is best-effort; fall through to live bytes.
    }

    for (const b of liveQueue) terminal.write(b);
    liveQueue.length = 0;
    replayed = true;
    // Do NOT force-scroll to bottom here: xterm already auto-scrolls as
    // the cursor moves during the backlog write, and forcing the viewport
    // strips the user's ability to scroll up into the main-buffer banner
    // (Claude's pre-alt-screen output) when the TUI later enters alt mode.

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
  <!-- Spacer decouples xterm's top edge from the session header.
       Using a sibling spacer instead of padding/margin means xterm's
       container fills its own box exactly — no reliance on padding math
       inside fit-addon's clientWidth/Height and no risk of xterm's
       internal absolute-positioned layers escaping the padded area. -->
  <div class="pane-inset-top" aria-hidden="true"></div>
  <div class="terminal-wrap">
    <div class="pane-inset-left" aria-hidden="true"></div>
    <div class="terminal-container" bind:this={containerEl} role="application" aria-label="Agent terminal output"></div>
  </div>
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

  /* Sibling spacers rather than padding/margin: xterm's container fills
     its own box exactly, so there's nothing for fit-addon or xterm's
     internal layers to miscalculate. */
  .pane-inset-top {
    flex: 0 0 24px;
  }

  .terminal-wrap {
    flex: 1;
    display: flex;
    flex-direction: row;
    min-height: 0;
    min-width: 0;
  }

  .pane-inset-left {
    flex: 0 0 10px;
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
