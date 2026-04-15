<!-- T065: Main page. Wires Sidebar + SessionList, hydrates stores on mount,
     and subscribes to backend events for real-time session updates. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionList from '$lib/components/SessionList.svelte';
  import AlertBar from '$lib/components/AlertBar.svelte';
  import SplitView from '$lib/components/SplitView.svelte';
  import CrossProjectOverview from '$lib/components/CrossProjectOverview.svelte';
  import SettingsDialog from '$lib/components/SettingsDialog.svelte';
  import SpawnSessionDialog from '$lib/components/SpawnSessionDialog.svelte';
  import LayoutSwitcher from '$lib/components/LayoutSwitcher.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { sessionSetFocus } from '$lib/api/sessions';
  import { scratchpadStore } from '$lib/stores/scratchpad.svelte';
  import { workspaceStore } from '$lib/stores/workspace.svelte';
  import { isEditableTarget } from '$lib/util/isEditableTarget';
  import type { Project } from '$lib/api/projects';
  import type { SessionSummary } from '$lib/api/sessions';

  let selectedProjectId = $state<number | null>(null);
  let activeSessionId = $state<number | null>(null);
  let settingsOpen = $state(false);
  let overviewOpen = $state(false);
  let spawnDialogOpen = $state(false);
  let spawnDialogProject = $state<Project | null>(null);
  /** P1-B: monotonic token that increments on every session activation. Passed
   *  to SplitView so it can re-trigger the 1.5 s border flash even when the
   *  user clicks an already-active session row (setting the same boolean
   *  `highlighted=true` twice wouldn't restart the CSS animation). Phase 4
   *  will expand this to support one token per slot. */
  let highlightToken = $state(0);
  /** Session id that was most recently activated. Only the pane rendering
   *  this session receives a non-zero token (so flashes don't bleed across
   *  slots once Phase 4 lands). */
  let highlightSessionId = $state<number | null>(null);

  /** P1-A: derived Set of session ids currently visible in a pane. Phase 1 is
   *  always the single active session; Phase 4 expands to the full slot set. */
  const activeSessionIds = $derived<Set<number>>(
    activeSessionId !== null ? new Set([activeSessionId]) : new Set(),
  );

  let sessionListRef = $state<{ focusFilter: () => void } | undefined>();

  function openSpawnDialog(project: Project | null = null): void {
    spawnDialogProject = project;
    spawnDialogOpen = true;
  }
  /** Session ids that were missing after workspace/layout restore. */
  let missingSessions = $state<Set<number>>(new Set());
  const activeSession = $derived(activeSessionId !== null ? sessionsStore.byId(activeSessionId) ?? null : null);

  /** Phase 2-D: project colour of the active session's project, threaded as
   *  `--project-color` on the active-session-header + SplitView wrapper so
   *  both the header tint and Phase 1's flash overlay use the same colour. */
  const activeProjectColor = $derived.by<string | null>(() => {
    if (!activeSession) return null;
    const color = projectsStore.byId(activeSession.project_id)?.settings?.color;
    return typeof color === 'string' ? color : null;
  });

  // Mirror activeSessionId to the backend so its event bridge stops
  // forwarding PTY output for sessions no pane can render. Overview /
  // no-selection → null → backend drops all Output/CompanionOutput events.
  $effect(() => {
    const id = activeSessionId;
    sessionSetFocus({ sessionId: overviewOpen ? null : id }).catch(() => {});
  });

  function handleSelectProject(project: Project): void {
    selectedProjectId = project.id;
    // L4: persist active project selection.
    workspaceStore.update({ active_project_ids: project.id !== null ? [project.id] : [] });
  }

  function handleActivateSession(session: SessionSummary): void {
    activeSessionId = session.id;
    overviewOpen = false;
    // T130: persist active session change.
    workspaceStore.update({ focused_session_id: session.id });

    // P1-B: Re-trigger the pane border flash. Incrementing the token — rather
    // than setting a boolean — ensures that clicking an already-active row
    // restarts the CSS animation (assigning the same boolean twice would not).
    // The SplitView keys its flash on this token, so a new value = new flash.
    highlightSessionId = session.id;
    highlightToken += 1;
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (
      event.key === '/' &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      !isEditableTarget(event.target)
    ) {
      event.preventDefault();
      sessionListRef?.focusFilter();
    }
  }

  onMount(() => {
    // T130: hydrate workspace first, then apply restored state, then other stores.
    const boot = (async () => {
      // 1. Workspace state first.
      await workspaceStore.hydrate();
      const ws = workspaceStore.current;

      // L4: restore active project + session from workspace state.
      if (ws.active_project_ids.length > 0) {
        selectedProjectId = ws.active_project_ids[0];
      }
      if (ws.focused_session_id !== null) {
        activeSessionId = ws.focused_session_id;
      }

      // 2. Projects + sessions in parallel.
      await Promise.all([
        projectsStore.hydrate({ includeArchived: true }),
        sessionsStore.hydrate(),
      ]);
    })();

    // Subscribe to real-time session events
    let cleanup: (() => void) | undefined;
    const subscribe = sessionsStore.subscribe().then((unsub) => {
      cleanup = unsub;
    });

    // M6: Use Tauri window close event so flush completes before exit.
    let closeCleanup: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const unlisten = await getCurrentWindow().onCloseRequested(async () => {
          await workspaceStore.flush();
        });
        closeCleanup = unlisten;
      } catch {
        // Not in Tauri context (dev/test) — onMount cleanup handles flush.
      }
    })();

    // L5: errors are surfaced via store .error fields; no console.warn.
    Promise.all([boot, subscribe]).catch(() => {});

    return () => {
      cleanup?.();
      closeCleanup?.();
      // Best-effort flush on unmount (fire-and-forget for non-Tauri contexts).
      workspaceStore.flush();
    };
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<div class="app-layout">
  <Sidebar
    {selectedProjectId}
    onSelectProject={handleSelectProject}
    onSpawnSession={(project) => openSpawnDialog(project)}
  />

  <main class="main-panel">
    <div class="session-panel">
      <div class="session-panel-header">
        <button
          class="overview-btn"
          onclick={() => { overviewOpen = !overviewOpen; activeSessionId = null; scratchpadStore.clear(); workspaceStore.update({ focused_session_id: null }); }}
          title="Cross-project reminder overview"
          aria-label="Open reminder overview"
          class:active={overviewOpen}
        >
          Overview
        </button>
        <LayoutSwitcher onMissingSessions={(ids) => { missingSessions = new Set(ids); }} />
        <button
          class="settings-btn"
          onclick={() => settingsOpen = true}
          title="Notification settings"
          aria-label="Open notification settings"
        >
          Settings
        </button>
      </div>
      <AlertBar onActivateSession={handleActivateSession} />
      <SessionList
        bind:this={sessionListRef}
        {selectedProjectId}
        {missingSessions}
        {activeSessionIds}
        onActivateSession={handleActivateSession}
        onSpawnSession={() => openSpawnDialog(
          selectedProjectId !== null
            ? projectsStore.byId(selectedProjectId) ?? null
            : null,
        )}
      />
    </div>

    <div class="content-panel">
      {#if overviewOpen}
        <CrossProjectOverview />
      {:else if activeSession && activeSessionId !== null}
        <div
          class="active-session-header"
          style={activeProjectColor ? `--project-color: ${activeProjectColor}` : undefined}
        >
          <h2>{activeSession.label}</h2>
          <span class="session-status">{activeSession.status}</span>
          {#if activeSession.ownership === 'wrapper' || activeSession.reattached_mirror}
            <span class="readonly-banner">Read-only</span>
          {/if}
        </div>
        {#key `${activeSessionId}-${activeSession.reattached_mirror}`}
          <!-- Phase 2-D: thread the project colour onto the SplitView wrapper
               so the Phase 1 flash overlay AND the companion pane header (via
               CSS cascade) both pick up the same per-project colour. -->
          <div
            class="split-view-wrapper"
            style={activeProjectColor ? `--project-color: ${activeProjectColor}` : undefined}
          >
            <SplitView
              sessionId={activeSessionId}
              session={activeSession}
              highlightToken={highlightSessionId === activeSessionId ? highlightToken : 0}
            />
          </div>
        {/key}
      {:else}
        <div class="empty-content">
          <h2>tend</h2>
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

<SettingsDialog open={settingsOpen} onclose={() => settingsOpen = false} />

<SpawnSessionDialog
  open={spawnDialogOpen}
  lockedProject={spawnDialogProject}
  onClose={() => (spawnDialogOpen = false)}
  onSpawned={(session) => {
    // Insert the session into the store immediately. The `session:spawned`
    // event will also fire and reconcile via hydrate, but doing it here
    // means the SessionList + SplitView render without waiting on the
    // event round-trip (which can race the dialog close).
    sessionsStore.add({
      ...session,
      activity_summary: null,
      alert: null,
      reattached_mirror: false,
    });
    // Activate the new session so the user lands directly in its terminal.
    activeSessionId = session.id;
    overviewOpen = false;
    workspaceStore.update({ focused_session_id: session.id });
    // Reconcile with the backend's full record (timestamps, metadata).
    sessionsStore.hydrate({ includeEnded: false }).catch(() => {});
  }}
/>

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
    display: flex;
    flex-direction: column;
  }

  .session-panel-header {
    display: flex;
    justify-content: flex-end;
    gap: 0.375rem;
    padding: 0.375rem 0.5rem;
    border-bottom: 1px solid var(--color-border, #2a2d35);
  }

  .overview-btn {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.75rem;
    margin-right: auto;
  }

  .overview-btn:hover {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
  }

  .overview-btn.active {
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
    border-color: var(--color-accent, #60a5fa);
  }

  .settings-btn {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.75rem;
  }

  .settings-btn:hover {
    background: var(--color-surface-hover, #1a1d25);
    color: var(--color-text, #e6e8ef);
  }

  .content-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  /* Phase 2-D: project-colour tint on the active-session header. A 4 px left
     strip carries the full project colour as an identity accent; the header
     background is a subtle 8% mix so the tint is visible on dark surfaces
     without reducing text contrast. Both fall back to `--color-accent` for
     projects without `settings.color`. */
  .active-session-header {
    display: flex;
    align-items: center;
    gap: var(--space-3, 0.75rem);
    padding: var(--space-3, 0.75rem) var(--space-4, 1rem);
    border-bottom: 1px solid var(--color-border, #2a2d35);
    border-left: 4px solid var(--project-color, var(--color-accent, #60a5fa));
    background: color-mix(
      in srgb,
      var(--project-color, var(--color-accent, #60a5fa)) 8%,
      var(--color-surface, #0f1115)
    );
  }

  /* Pass-through wrapper for the project-colour CSS variable. The SplitView
     reads `--project-color` from its closest ancestor (the Phase 1 flash
     overlay + any future companion-pane tinting) — putting it on a neutral
     wrapper keeps SplitView itself layout-agnostic. */
  .split-view-wrapper {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
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
    background: var(--color-warning-bg, #3d2e00);
    color: var(--color-warning, #fbbf24);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
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
