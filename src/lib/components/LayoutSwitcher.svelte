<!-- T129: Layout switcher — dropdown listing saved layouts with save/delete.
     M3: click-outside-to-close. M4: keyboard nav (Escape, ArrowUp/Down). -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    layoutList,
    layoutSave,
    layoutRestore,
    layoutDelete,
    type Layout,
    type WorkspaceState,
  } from '$lib/api/workspace';
  import { WorkbenchError } from '$lib/api/invoke';
  import { workspaceStore } from '$lib/stores/workspace.svelte';
  import SpinnerIcon from '$lib/components/SpinnerIcon.svelte';

  interface Props {
    onMissingSessions?: (ids: number[]) => void;
  }

  let { onMissingSessions }: Props = $props();

  let layouts = $state<Layout[]>([]);
  let open = $state(false);
  let saveName = $state('');
  let saving = $state(false);
  /** P1-D: true while the layouts list is being re-fetched. */
  let refreshing = $state(false);
  let error = $state<string | null>(null);
  let rootEl: HTMLDivElement | undefined = $state();
  let triggerEl: HTMLButtonElement | undefined = $state();
  /** Index of the focused layout item for roving tabindex (-1 = none). */
  let focusedIndex = $state(-1);

  async function refresh(): Promise<void> {
    refreshing = true;
    try {
      const result = await layoutList();
      layouts = result.layouts;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      refreshing = false;
    }
  }

  async function handleSave(): Promise<void> {
    const name = saveName.trim();
    if (!name) return;
    saving = true;
    error = null;
    try {
      await layoutSave(name, workspaceStore.current);
      saveName = '';
      await refresh();
    } catch (err) {
      if (err instanceof WorkbenchError && err.code === 'NAME_TAKEN') {
        const confirmed = confirm(`Layout "${name}" already exists. Overwrite?`);
        if (confirmed) {
          try {
            await layoutSave(name, workspaceStore.current, true);
            saveName = '';
            await refresh();
          } catch (retryErr) {
            error = retryErr instanceof Error ? retryErr.message : String(retryErr);
          }
        }
      } else {
        error = err instanceof Error ? err.message : String(err);
      }
    } finally {
      saving = false;
    }
  }

  async function handleRestore(id: number): Promise<void> {
    error = null;
    try {
      const result = await layoutRestore(id);
      workspaceStore.replace(result.state);
      if (result.missing_sessions.length > 0) {
        error = `${result.missing_sessions.length} session(s) no longer running`;
        onMissingSessions?.(result.missing_sessions);
      }
      close();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleDelete(id: number): Promise<void> {
    error = null;
    try {
      await layoutDelete(id);
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function toggle(): void {
    if (open) {
      close();
    } else {
      open = true;
      focusedIndex = -1;
      refresh();
    }
  }

  function close(): void {
    open = false;
    focusedIndex = -1;
    triggerEl?.focus();
  }

  // M3: click-outside-to-close.
  function handleDocumentClick(event: MouseEvent): void {
    if (open && rootEl && !rootEl.contains(event.target as Node)) {
      close();
    }
  }

  // M4: keyboard navigation on the dropdown.
  function handleDropdownKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      close();
      return;
    }
    if (!layouts.length) return;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      focusedIndex = Math.min(focusedIndex + 1, layouts.length - 1);
      focusLayoutItem();
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      focusedIndex = Math.max(focusedIndex - 1, 0);
      focusLayoutItem();
    } else if (event.key === 'Home') {
      event.preventDefault();
      focusedIndex = 0;
      focusLayoutItem();
    } else if (event.key === 'End') {
      event.preventDefault();
      focusedIndex = layouts.length - 1;
      focusLayoutItem();
    }
  }

  function focusLayoutItem(): void {
    if (focusedIndex < 0 || !rootEl) return;
    const items = rootEl.querySelectorAll<HTMLButtonElement>('.layout-name');
    items[focusedIndex]?.focus();
  }

  onMount(() => {
    document.addEventListener('click', handleDocumentClick, true);
  });

  onDestroy(() => {
    document.removeEventListener('click', handleDocumentClick, true);
  });
</script>

<div class="layout-switcher" bind:this={rootEl}>
  <button
    class="layout-trigger"
    bind:this={triggerEl}
    onclick={toggle}
    aria-expanded={open}
    aria-haspopup="menu"
    title="Workspace layouts"
    aria-label="Workspace layouts"
  >
    Layouts
  </button>

  {#if open}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="layout-dropdown" role="menu" tabindex="-1" onkeydown={handleDropdownKeydown}>
      <div class="layout-header">
        <span class="layout-header-label">Layouts</span>
        {#if refreshing}
          <SpinnerIcon />
        {/if}
      </div>
      {#if error}
        <div class="layout-error" role="alert">{error}</div>
      {/if}

      <div class="layout-save-row">
        <input
          type="text"
          bind:value={saveName}
          placeholder="Save current as…"
          aria-label="Layout name"
          maxlength={255}
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') handleSave(); if (e.key === 'Escape') close(); }}
        />
        <button
          onclick={handleSave}
          disabled={saving || !saveName.trim()}
          aria-label="Save layout"
        >
          Save
        </button>
      </div>

      {#if layouts.length > 0}
        <div class="layout-list" role="group" aria-label="Saved layouts">
          {#each layouts as layout, i (layout.id)}
            <div class="layout-item">
              <button
                class="layout-name"
                onclick={() => handleRestore(layout.id)}
                title="Restore {layout.name}"
                role="menuitem"
                tabindex={focusedIndex === i ? 0 : -1}
              >
                {layout.name}
              </button>
              <button
                class="layout-delete"
                onclick={() => handleDelete(layout.id)}
                aria-label="Delete layout {layout.name}"
                title="Delete"
                tabindex={-1}
              >
                &times;
              </button>
            </div>
          {/each}
        </div>
      {:else if refreshing}
        <p class="layout-empty">
          <SpinnerIcon />
          <span>Loading…</span>
        </p>
      {:else}
        <p class="layout-empty">No saved layouts</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .layout-switcher {
    position: relative;
  }

  .layout-trigger {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.75rem;
  }

  .layout-trigger:hover {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
  }

  .layout-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    z-index: 100;
    min-width: 220px;
    margin-top: 0.25rem;
    padding: 0.5rem;
    background: var(--color-surface, #0f1115);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .layout-header {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0 0.25rem 0.375rem;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .layout-header-label {
    flex: 1;
  }

  .layout-error {
    padding: 0.25rem 0.5rem;
    margin-bottom: 0.375rem;
    font-size: 0.6875rem;
    color: var(--color-error, #ef4444);
    background: var(--color-error-bg, rgba(239, 68, 68, 0.1));
    border-radius: 0.25rem;
  }

  .layout-save-row {
    display: flex;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
  }

  .layout-save-row input {
    flex: 1;
    padding: 0.25rem 0.375rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
    font-size: 0.75rem;
  }

  .layout-save-row button {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
    font-size: 0.75rem;
    cursor: pointer;
    font-weight: 600;
  }

  .layout-save-row button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .layout-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .layout-item {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0;
  }

  .layout-name {
    flex: 1;
    padding: 0.25rem 0.375rem;
    border: none;
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text, #e6e8ef);
    font-size: 0.75rem;
    cursor: pointer;
    text-align: left;
  }

  .layout-name:hover,
  .layout-name:focus-visible {
    background: var(--color-surface-hover, #1a1d25);
    outline: 1px solid var(--color-accent, #60a5fa);
  }

  .layout-delete {
    padding: 0.125rem 0.375rem;
    border: none;
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.875rem;
    line-height: 1;
  }

  .layout-delete:hover {
    background: var(--color-error-bg, rgba(239, 68, 68, 0.1));
    color: var(--color-error, #ef4444);
  }

  .layout-empty {
    margin: 0;
    padding: 0.5rem 0.375rem;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.75rem;
    text-align: center;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
  }
</style>
