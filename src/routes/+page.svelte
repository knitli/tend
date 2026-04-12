<!-- T065: Main page. Wires Sidebar + SessionList, hydrates stores on mount,
     and subscribes to backend events for real-time session updates. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionList from '$lib/components/SessionList.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import type { Project } from '$lib/api/projects';
  import type { SessionSummary } from '$lib/api/sessions';

  let selectedProjectId = $state<number | null>(null);
  let activeSessionId = $state<number | null>(null);
  const activeSession = $derived(activeSessionId !== null ? sessionsStore.byId(activeSessionId) ?? null : null);

  function handleSelectProject(project: Project): void {
    selectedProjectId = project.id;
  }

  function handleActivateSession(session: SessionSummary): void {
    activeSessionId = session.id;
  }

  onMount(() => {
    // Hydrate both stores in parallel on mount
    const hydrate = Promise.all([
      projectsStore.hydrate({ includeArchived: true }),
      sessionsStore.hydrate(),
    ]);

    // Subscribe to real-time session events
    let cleanup: (() => void) | undefined;
    const subscribe = sessionsStore.subscribe().then((unsub) => {
      cleanup = unsub;
    });

    // Log hydration errors for debugging (non-blocking)
    Promise.all([hydrate, subscribe]).catch((err) => {
      // Tauri invoke will fail in dev mode without a running backend;
      // stores surface the error through their .error fields.
      console.warn('Store hydration/subscription failed:', err);
    });

    return () => {
      cleanup?.();
    };
  });
</script>

<div class="app-layout">
  <Sidebar
    {selectedProjectId}
    onSelectProject={handleSelectProject}
  />

  <main class="main-panel">
    <div class="session-panel">
      <SessionList
        {selectedProjectId}
        onActivateSession={handleActivateSession}
      />
    </div>

    <div class="content-panel">
      {#if activeSession}
        <div class="active-session-header">
          <h2>{activeSession.label}</h2>
          <span class="session-status">{activeSession.status}</span>
          {#if activeSession.ownership === 'wrapper' || activeSession.reattached_mirror}
            <span class="readonly-banner">Read-only</span>
          {/if}
        </div>
        <div class="terminal-placeholder">
          <p class="muted">
            Terminal pane will be wired in the activation task (T085+).
          </p>
        </div>
      {:else}
        <div class="empty-content">
          <h2>agentui</h2>
          <p class="muted">
            Select a session from the list to view its terminal output.
          </p>
          {#if projectsStore.activeProjects.length === 0 && !projectsStore.loading}
            <p class="muted">
              Add a project in the sidebar to get started.
            </p>
          {/if}
        </div>
      {/if}
    </div>
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
  }

  .main-panel {
    display: flex;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .session-panel {
    width: 320px;
    min-width: 240px;
    border-right: 1px solid var(--color-border, #2a2d35);
    overflow: hidden;
  }

  .content-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .active-session-header {
    display: flex;
    align-items: center;
    gap: var(--space-3, 0.75rem);
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .active-session-header h2 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
  }

  .session-status {
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
    text-transform: capitalize;
  }

  .readonly-banner {
    padding: 2px 8px;
    border-radius: var(--radius-sm, 4px);
    background: var(--color-warning-bg, #713f12);
    color: var(--color-warning, #fbbf24);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .terminal-placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-6, 1.5rem);
  }

  .empty-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-6, 1.5rem);
  }

  .empty-content h2 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 300;
    color: var(--color-text-muted, #8b8fa3);
  }

  .muted {
    margin: 0;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.875rem;
  }
</style>
