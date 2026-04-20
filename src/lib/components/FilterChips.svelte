<!--
  Agent-kind filter chips. "All" shows every session; selecting a specific
  kind narrows to sessions whose `metadata.command` / label matches.
  The selected kind is owned by the parent — this component is a controlled
  toggle strip.
-->
<script lang="ts">
  import { AGENT_KIND_META, type AgentKind } from '$lib/util/agentKind';

  interface Props {
    /** Currently-selected kind, or `null` for "All". */
    value: AgentKind | null;
    onChange: (next: AgentKind | null) => void;
  }

  let { value, onChange }: Props = $props();
</script>

<div class="filter-chips" role="group" aria-label="Filter by agent">
  <button
    type="button"
    class="chip chip-all"
    class:active={value === null}
    aria-pressed={value === null}
    onclick={() => onChange(null)}
  >
    All
  </button>
  {#each AGENT_KIND_META as meta (meta.id)}
    <button
      type="button"
      class="chip"
      class:active={value === meta.id}
      style="--chip-color: {meta.color}"
      aria-pressed={value === meta.id}
      onclick={() => onChange(meta.id)}
    >
      <span class="dot" aria-hidden="true"></span>
      {meta.label}
    </button>
  {/each}
</div>

<style>
  .filter-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.25rem 0.75rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 999px;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.75rem;
    font-family: inherit;
    cursor: pointer;
    transition: background 150ms, color 150ms, border-color 150ms;
  }

  .chip:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .chip:focus-visible {
    outline: 2px solid var(--color-accent, #60a5fa);
    outline-offset: 2px;
  }

  .chip.active {
    background: var(--color-surface-active, #252830);
    color: var(--color-text, #e6e8ef);
    border-color: var(--chip-color, var(--color-accent, #60a5fa));
  }

  .chip.chip-all.active {
    background: var(--color-surface-active, #252830);
    border-color: var(--color-text, #e6e8ef);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--chip-color, var(--color-accent, #60a5fa));
    flex-shrink: 0;
  }
</style>
