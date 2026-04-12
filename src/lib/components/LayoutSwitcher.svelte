<!-- T129: Layout switcher — dropdown listing saved layouts with save/delete. -->
<script lang="ts">
  import {
    layoutList,
    layoutSave,
    layoutRestore,
    layoutDelete,
    type Layout,
    type WorkspaceState,
  } from '$lib/api/workspace';
  import { workspaceStore } from '$lib/stores/workspace.svelte';

  let layouts = $state<Layout[]>([]);
  let open = $state(false);
  let saveName = $state('');
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function refresh(): Promise<void> {
    try {
      const result = await layoutList();
      layouts = result.layouts;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
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
      error = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }

  async function handleRestore(id: number): Promise<void> {
    error = null;
    try {
      const result = await layoutRestore(id);
      workspaceStore.update(result.state);
      if (result.missing_sessions.length > 0) {
        error = `${result.missing_sessions.length} session(s) no longer running`;
      }
      open = false;
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
    open = !open;
    if (open) {
      refresh();
    }
  }
</script>

<div class="layout-switcher">
  <button
    class="layout-trigger"
    onclick={toggle}
    aria-expanded={open}
    aria-haspopup="true"
    title="Workspace layouts"
    aria-label="Workspace layouts"
  >
    Layouts
  </button>

  {#if open}
    <div class="layout-dropdown" role="menu">
      {#if error}
        <div class="layout-error" role="alert">{error}</div>
      {/if}

      <div class="layout-save-row">
        <input
          type="text"
          bind:value={saveName}
          placeholder="Save current as…"
          aria-label="Layout name"
          onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') handleSave(); }}
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
        <ul class="layout-list" role="list">
          {#each layouts as layout (layout.id)}
            <li class="layout-item" role="menuitem">
              <button
                class="layout-name"
                onclick={() => handleRestore(layout.id)}
                title="Restore {layout.name}"
              >
                {layout.name}
              </button>
              <button
                class="layout-delete"
                onclick={() => handleDelete(layout.id)}
                aria-label="Delete layout {layout.name}"
                title="Delete"
              >
                &times;
              </button>
            </li>
          {/each}
        </ul>
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

  .layout-name:hover {
    background: var(--color-surface-hover, #1a1d25);
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
  }
</style>
