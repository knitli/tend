<!--
  Shared page header used by the full-width pages (Dashboard, Sessions,
  Projects, Settings, Workspaces). Title + optional subtitle on the left,
  free-form trailing content (buttons, etc.) on the right via snippet.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    subtitle?: string;
    trailing?: Snippet;
  }

  let { title, subtitle, trailing }: Props = $props();
</script>

<header class="page-header">
  <div class="titles">
    <h1>{title}</h1>
    {#if subtitle}<p class="subtitle">{subtitle}</p>{/if}
  </div>
  {#if trailing}
    <div class="trailing">{@render trailing()}</div>
  {/if}
</header>

<style>
  .page-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding: 1.25rem 1.5rem 0.75rem;
  }

  .titles {
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: 1.375rem;
    font-weight: 700;
    color: var(--color-text, #e6e8ef);
    letter-spacing: -0.01em;
  }

  .subtitle {
    margin: 0.25rem 0 0;
    font-size: 0.8125rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .trailing {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
</style>
