<script lang="ts">
  import { onMount } from "svelte";
  import { createTerminal, type CreatedTerminal } from "$lib/xterm/createTerminal";

  // Phase 2 smoke test: mount one xterm instance and write "workbench ready"
  // so visual regressions in the dev server immediately show up. US1 will
  // replace this with the real `Sidebar` + `SessionList` wiring (T065).
  let host: HTMLDivElement | undefined;
  let term: CreatedTerminal | undefined;

  onMount(() => {
    if (!host) return;
    term = createTerminal(host);
    term.terminal.write("workbench ready\r\n");
    return () => term?.dispose();
  });
</script>

<main>
  <h1>agentui</h1>
  <p class="muted">
    Phase 2 scaffold. Not yet functional — US1 (tasks T031–T065) delivers the
    unified session overview MVP.
  </p>
  <div class="term" bind:this={host}></div>
</main>

<style>
  main {
    padding: var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    height: 100%;
  }
  h1 {
    margin: 0;
    font-size: 1.5rem;
  }
  .muted {
    margin: 0;
    color: var(--color-text-muted);
  }
  .term {
    flex: 1;
    min-height: 300px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-2);
  }
</style>
