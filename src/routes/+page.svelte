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
  import HamburgerButton from '$lib/components/HamburgerButton.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { sessionSetFocus } from '$lib/api/sessions';
  import { scratchpadStore } from '$lib/stores/scratchpad.svelte';
  import { workspaceStore } from '$lib/stores/workspace.svelte';
  import { isEditableTarget } from '$lib/util/isEditableTarget';
  import type { Project } from '$lib/api/projects';
  import type { SessionSummary } from '$lib/api/sessions';
  import { getProjectColor } from '$lib/util/projectColor';

  let selectedProjectId = $state<number | null>(null);
  let activeSessionId = $state<number | null>(null);
  let settingsOpen = $state(false);
  let overviewOpen = $state(false);
  let spawnDialogOpen = $state(false);
  let spawnDialogProject = $state<Project | null>(null);

  /** P3-A: sidebar collapse state. Hydrated from `workspace.ui.sidebar_collapsed`
   *  on mount (see onMount below) and persisted on every toggle. */
  let sidebarCollapsed = $state(false);
  /** P3-A: hover-peek state. When the sidebar is collapsed AND the user is
   *  hovering the left-edge hot zone (or the peeked sidebar itself), the
   *  sidebar slides out as an *overlay* (position: absolute) so the content
   *  area is not compressed. A short leave delay gives the cursor time to
   *  cross from the hot zone onto the sidebar without collapsing. */
  let sidebarPeeking = $state(false);
  let peekLeaveTimer: ReturnType<typeof setTimeout> | null = null;

  /** P3-B: focus mode state. `'single'` hides both sidebars and fills the
   *  content area with one session's SplitView. `'split-two'` is declared for
   *  forward-compat with Phase 4's multi-pane work — entry paths are not
   *  wired in Phase 3, so a persisted `split-two` state is rendered as
   *  `single` on the first id in focusSessionIds. */
  let focusMode = $state<'none' | 'single' | 'split-two'>('none');
  let focusSessionIds = $state<number[]>([]);
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
    return getProjectColor(projectsStore.byId(activeSession.project_id));
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
      return;
    }

    // P3-B: Ctrl+Shift+F toggles single-session focus mode on the active
    // session. Guarded with isEditableTarget so it doesn't hijack while the
    // user is typing in a text input.
    if (
      event.key === 'F' &&
      event.ctrlKey &&
      event.shiftKey &&
      !event.metaKey &&
      !event.altKey &&
      !isEditableTarget(event.target)
    ) {
      event.preventDefault();
      if (focusMode === 'none' && activeSessionId !== null) {
        enterFocusMode(activeSessionId);
      } else if (focusMode !== 'none') {
        exitFocusMode();
      }
      return;
    }

    // P3-B: Escape exits focus mode. CRITICAL: isEditableTarget also returns
    // true when the event target is inside xterm's `.xterm-helper-textarea`,
    // so Escape inside a terminal is preserved for xterm's own handling
    // (Claude Code uses Escape to interrupt, for example).
    if (event.key === 'Escape' && focusMode !== 'none' && !isEditableTarget(event.target)) {
      event.preventDefault();
      exitFocusMode();
      return;
    }
  }

  // ─── P3-A sidebar helpers ──────────────────────────────────────────────
  function toggleSidebar(nextOpen: boolean): void {
    sidebarCollapsed = !nextOpen;
    workspaceStore.setUi('sidebar_collapsed', sidebarCollapsed);
    // Closing the sidebar also cancels any in-flight peek.
    if (sidebarCollapsed === false) {
      sidebarPeeking = false;
      if (peekLeaveTimer !== null) {
        clearTimeout(peekLeaveTimer);
        peekLeaveTimer = null;
      }
    }
  }

  function onPeekEnter(): void {
    if (!sidebarCollapsed) return;
    if (peekLeaveTimer !== null) {
      clearTimeout(peekLeaveTimer);
      peekLeaveTimer = null;
    }
    sidebarPeeking = true;
  }

  function onPeekLeave(): void {
    if (!sidebarCollapsed) return;
    // Delay the collapse so the cursor can cross from the hot zone onto the
    // peeked sidebar overlay (and vice versa) without flicker.
    if (peekLeaveTimer !== null) clearTimeout(peekLeaveTimer);
    peekLeaveTimer = setTimeout(() => {
      sidebarPeeking = false;
      peekLeaveTimer = null;
    }, 300);
  }

  // ─── P3-B focus mode helpers ──────────────────────────────────────────
  function enterFocusMode(sessionId: number): void {
    focusMode = 'single';
    focusSessionIds = [sessionId];
    // Ensure the active session matches so the content panel renders the
    // right SplitView.
    if (activeSessionId !== sessionId) {
      activeSessionId = sessionId;
      workspaceStore.update({ focused_session_id: sessionId });
    }
    workspaceStore.setUi('focus_mode', focusMode);
    workspaceStore.setUi('focus_mode_session_ids', focusSessionIds);
  }

  function exitFocusMode(): void {
    focusMode = 'none';
    focusSessionIds = [];
    workspaceStore.setUi('focus_mode', focusMode);
    workspaceStore.setUi('focus_mode_session_ids', focusSessionIds);
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

      // P3-A / P3-B: restore UI state from the persisted ui map.
      if (typeof ws.ui?.sidebar_collapsed === 'boolean') {
        sidebarCollapsed = ws.ui.sidebar_collapsed;
      }
      if (
        ws.ui?.focus_mode === 'single' ||
        ws.ui?.focus_mode === 'split-two' ||
        ws.ui?.focus_mode === 'none'
      ) {
        focusMode = ws.ui.focus_mode;
      }
      if (Array.isArray(ws.ui?.focus_mode_session_ids)) {
        focusSessionIds = ws.ui.focus_mode_session_ids.filter(
          (v): v is number => typeof v === 'number',
        );
      }
      // split-two is declared in the type but Phase 3 only renders single. If
      // a persisted layout had split-two, degrade to single on the first id.
      if (focusMode === 'split-two') {
        focusMode = focusSessionIds.length > 0 ? 'single' : 'none';
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
      if (peekLeaveTimer !== null) {
        clearTimeout(peekLeaveTimer);
        peekLeaveTimer = null;
      }
      // Best-effort flush on unmount (fire-and-forget for non-Tauri contexts).
      workspaceStore.flush();
    };
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<div
  class="app-layout"
  class:focus-mode={focusMode !== 'none'}
  class:sidebar-collapsed={sidebarCollapsed}
>
  <Sidebar
    {selectedProjectId}
    onSelectProject={handleSelectProject}
    onSpawnSession={(project) => openSpawnDialog(project)}
    open={!sidebarCollapsed || sidebarPeeking}
    peeking={sidebarCollapsed && sidebarPeeking}
    contentId="sidebar-collapsible-content"
  />

  <main class="main-panel">
    <!-- P3-A: hamburger button always visible; toggles the Collapsible's open state. -->
    <div class="hamburger-slot">
      <HamburgerButton
        open={!sidebarCollapsed}
        controlsId="sidebar-collapsible-content"
        onToggle={toggleSidebar}
      />
    </div>

    <!-- P3-A: transparent hover hot zone on the left edge of the main panel.
         Only active when the sidebar is collapsed — triggers the peek overlay. -->
    {#if sidebarCollapsed}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="sidebar-peek-hotzone"
        aria-hidden="true"
        onmouseenter={onPeekEnter}
        onmouseleave={onPeekLeave}
      ></div>
      {#if sidebarPeeking}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="sidebar-peek-zone"
          aria-hidden="true"
          onmouseenter={onPeekEnter}
          onmouseleave={onPeekLeave}
        ></div>
      {/if}
    {/if}

    <!-- P3-B: AlertBar lives at the top of .main-panel (not inside the session
         panel) so it stays visible in focus mode. Alerts must never be hidden. -->
    <AlertBar onActivateSession={handleActivateSession} />

    <div class="main-panel-body">
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
          <!-- P3-B: In focus mode, replace the normal header with a compact
               breadcrumb chip so the user retains orientation. The exit `×`
               button is always rendered when in focus mode so it remains
               clickable even while xterm has keyboard focus. -->
          {#if focusMode !== 'none'}
            <div
              class="focus-breadcrumb"
              style={activeProjectColor ? `--project-color: ${activeProjectColor}` : ''}
            >
              <span class="focus-dot" aria-hidden="true"></span>
              <span class="focus-breadcrumb-text">
                <strong>{projectsStore.byId(activeSession.project_id)?.display_name ?? 'Project'}</strong>
                <span class="focus-separator">/</span>
                {activeSession.label}
                <span class="focus-separator">·</span>
                <span class="focus-status">{activeSession.status}</span>
              </span>
              {#if activeSession.ownership === 'wrapper' || activeSession.reattached_mirror}
                <span class="readonly-banner">Read-only</span>
              {/if}
            </div>
            <button
              type="button"
              class="focus-exit-btn"
              onclick={exitFocusMode}
              title="Exit focus mode (Esc)"
              aria-label="Exit focus mode"
            >
              ×
            </button>
          {:else}
            <div
              class="active-session-header"
              style={activeProjectColor ? `--project-color: ${activeProjectColor}` : ''}
            >
              <h2>{activeSession.label}</h2>
              <span class="session-status">{activeSession.status}</span>
              {#if activeSession.ownership === 'wrapper' || activeSession.reattached_mirror}
                <span class="readonly-banner">Read-only</span>
              {/if}
              <button
                type="button"
                class="focus-enter-btn"
                onclick={() => enterFocusMode(activeSessionId!)}
                title="Focus on this session (Ctrl+Shift+F)"
                aria-label="Focus on this session"
              >
                ⊙
              </button>
            </div>
          {/if}
          {#key `${activeSessionId}-${activeSession.reattached_mirror}`}
            <!-- Phase 2-D: thread the project colour onto the SplitView wrapper
                 so the Phase 1 flash overlay AND the companion pane header (via
                 CSS cascade) both pick up the same per-project colour. -->
            <div
              class="split-view-wrapper"
              style={activeProjectColor ? `--project-color: ${activeProjectColor}` : ''}
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
    position: relative;
  }

  /* Root layer needed so the sidebar's peek overlay (position: absolute) can
     anchor to the main-panel rather than the collapsed sidebar. */
  .main-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    position: relative;
  }

  .main-panel-body {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  /* P3-A: hamburger button floats over the top-left of the main panel so it
     remains reachable when the sidebar is collapsed to zero width. z-index
     sits above the peek overlay (50) so the button is always on top. */
  .hamburger-slot {
    position: absolute;
    top: 4px;
    left: 4px;
    z-index: 60;
  }

  /* P3-A: invisible hover hot zone on the left edge. Activates the peek
     overlay when the sidebar is collapsed. */
  .sidebar-peek-hotzone {
    position: absolute;
    top: 0;
    left: 0;
    width: 48px;
    height: 100%;
    z-index: 30;
    cursor: pointer;
  }

  /* P3-A: when peek is active, extend the mouse-leave-safe zone to cover the
     260 px peeked sidebar too. Prevents flicker when the cursor transitions
     from the hot zone onto the peeked sidebar. */
  .sidebar-peek-zone {
    position: absolute;
    top: 0;
    left: 0;
    width: 260px;
    height: 100%;
    z-index: 40;
    pointer-events: auto;
  }

  /* P3-B: focus mode hides the session panel with a width transition. The
     collapsed sidebar is already controlled by bits-ui; we force it closed
     here so the content area expands to full width. */
  .app-layout.focus-mode :global(.sidebar-collapsible[data-state="open"]),
  .app-layout.focus-mode :global(.sidebar-collapsible[data-state="closed"]) {
    width: 0;
    border-right: 1px solid transparent;
    overflow: hidden;
  }

  .app-layout.focus-mode .session-panel {
    width: 0;
    min-width: 0;
    border-right: none;
    overflow: hidden;
    opacity: 0;
    transition: width 200ms ease, opacity 200ms ease;
  }

  .app-layout:not(.focus-mode) .session-panel {
    transition: width 200ms ease, opacity 200ms ease;
  }

  @media (prefers-reduced-motion: reduce) {
    .app-layout.focus-mode .session-panel,
    .app-layout:not(.focus-mode) .session-panel {
      transition: none;
    }
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
    position: relative;
  }

  /* P3-B: compact breadcrumb chip shown at the top of the content panel when
     focus mode is active. Replaces the larger active-session-header so the
     user retains orientation ("I'm in session X of project Y") without the
     full header chrome. Matches the NNGroup breadcrumb pattern cited in
     research.md §B.5. */
  .focus-breadcrumb {
    display: flex;
    align-items: center;
    gap: var(--space-2, 0.5rem);
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    padding-right: 40px; /* leave room for focus-exit-btn */
    border-bottom: 1px solid var(--color-border, #2a2d35);
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .focus-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    flex-shrink: 0;
  }

  .focus-breadcrumb-text {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    flex: 1;
    min-width: 0;
    color: var(--color-text, #e6e8ef);
  }

  .focus-breadcrumb-text strong {
    font-weight: 600;
  }

  .focus-separator {
    color: var(--color-text-muted, #8b8fa3);
    opacity: 0.6;
  }

  .focus-status {
    color: var(--color-text-muted, #8b8fa3);
    text-transform: capitalize;
  }

  /* P3-B: always-visible exit button. Positioned absolute so it's reachable
     even when xterm has keyboard focus (xterm captures Escape and other
     keys — a visible click target is required per research §B.5 Replit). */
  .focus-exit-btn {
    position: absolute;
    top: 4px;
    right: 4px;
    z-index: 60;
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 1.125rem;
    line-height: 1;
    cursor: pointer;
    opacity: 0.6;
    transition: opacity 150ms, background 150ms, color 150ms;
  }

  .focus-exit-btn:hover,
  .focus-exit-btn:focus-visible {
    opacity: 1;
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  /* P3-B: maximize button that enters focus mode on the active session. */
  .focus-enter-btn {
    margin-left: auto;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.875rem;
    line-height: 1;
    cursor: pointer;
    transition: background 150ms, color 150ms;
  }

  .focus-enter-btn:hover,
  .focus-enter-btn:focus-visible {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
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

  /* Layout carrier for the SplitView: `flex: 1; min-height: 0;
     overflow: hidden;` give the split its bounding box inside the content
     panel. This wrapper ALSO threads `--project-color` via its inline
     `style` attribute, so the Phase 1 flash overlay and any future
     companion-pane tinting can read the per-project colour from the
     closest ancestor while SplitView itself stays layout-agnostic. */
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
