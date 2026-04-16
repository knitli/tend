<!-- T065: Main page. Wires Sidebar + SessionList, hydrates stores on mount,
     and subscribes to backend events for real-time session updates. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import SessionList from '$lib/components/SessionList.svelte';
  import AlertBar from '$lib/components/AlertBar.svelte';
  import SplitView from '$lib/components/SplitView.svelte';
  import PaneWorkspace from '$lib/components/PaneWorkspace.svelte';
  import CrossProjectOverview from '$lib/components/CrossProjectOverview.svelte';
  import SettingsDialog from '$lib/components/SettingsDialog.svelte';
  import SpawnSessionDialog from '$lib/components/SpawnSessionDialog.svelte';
  import LayoutSwitcher from '$lib/components/LayoutSwitcher.svelte';
  // HamburgerButton removed — sidebar edge toggle handles collapse/expand.
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import MainTabs, { type TabId } from '$lib/components/MainTabs.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { sessionSetVisible, sessionSpawn } from '$lib/api/sessions';
  import { workspaceStore } from '$lib/stores/workspace.svelte';
  import { isEditableTarget } from '$lib/util/isEditableTarget';
  import type { Project } from '$lib/api/projects';
  import type { SessionSummary } from '$lib/api/sessions';
  import type { GhostSessionData, PaneSlot } from '$lib/types/pane';
  import { getProjectColor } from '$lib/util/projectColor';

  let selectedProjectId = $state<number | null>(null);
  let activeSessionId = $state<number | null>(null);
  let settingsOpen = $state(false);
  /** P4-F: replaces the old `overviewOpen` boolean. The main panel is now
   *  divided into three top-level tabs (Sessions / Workspace / Overview)
   *  persisted via `workspace.ui.active_view`. Overview is no longer a
   *  toggle button inside the session panel. */
  let activeView = $state<TabId>('sessions');
  let spawnDialogOpen = $state(false);
  let spawnDialogProject = $state<Project | null>(null);
  /** P4-E: Ctrl+K / Cmd+K quick-switch palette visibility. */
  let paletteOpen = $state(false);
  /** P3-A: sidebar collapse state. Hydrated from `workspace.ui.sidebar_collapsed`
   *  on mount (see onMount below) and persisted on every toggle. */
  let sidebarCollapsed = $state(false);
  // Peek state removed — sidebar now uses a visible edge toggle instead.

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

  /** Phase 4-B/C: horizontal pane-workspace slots. Derived from either
   *  `activeSessionId` (single-session compat path) or rehydrated from
   *  `workspace.ui.workspace_pane_slots` on mount. Every mutation is
   *  persisted via `workspaceStore.setUi` so layouts survive restarts.
   *
   *  The `activeSessionId` scalar is retained as a compatibility shim:
   *  - spawn dialog / session-row click set it and we mirror into the
   *    slot set via `handleActivateSession`;
   *  - focus-mode and no-slot paths still derive visibility from it in the
   *    `sessionSetVisible` effect below when PaneWorkspace is not mounted.
   */
  let slots = $state<PaneSlot[]>([]);
  /** Persisted per-pane size percentages (paneforge onLayoutChange). */
  let paneSizes = $state<number[] | undefined>(undefined);

  /** P1-A: Set of session ids currently rendered by a pane. Phase 4 unions
   *  every visible slot so the SessionList's active-row indicator lights
   *  up for every pane, not just `activeSessionId`. Falls back to the
   *  scalar when no slots are open yet (pre-hydrate / empty state). */
  const activeSessionIds = $derived<Set<number>>(
    slots.length > 0
      ? new Set(slots.map((s) => s.session_id))
      : activeSessionId !== null
        ? new Set([activeSessionId])
        : new Set(),
  );

  /** `bind:this` is set on BOTH the Sessions-tab and Workspace-tab SessionList
   *  renders below. This is safe because MainTabs gates each snippet with
   *  `{#if value === '...'}`, so only the active tab mounts its SessionList at
   *  any time — the two bindings never write to `sessionListRef` simultaneously.
   *  Focus mode renders neither SessionList (MainTabs itself is hidden), so
   *  `sessionListRef` is simply `undefined` there and `focusFilter()` is a
   *  no-op via optional chaining. */
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

  // Phase 4-B/C: PaneWorkspace now owns `session_set_visible` and calls it
  // with every visible slot id. The old single-session `sessionSetFocus`
  // effect was REMOVED because it was a backend shim that overwrote the
  // visible set with a single id, racing PaneWorkspace's multi-id update.
  //
  // When overview is open or no slots are mounted, PaneWorkspace isn't
  // rendered — so we mirror an empty visible set here to silence PTY
  // forwarding for non-rendered sessions. (Focus mode also renders
  // SplitView directly outside PaneWorkspace, so it needs its own
  // single-id call.)
  $effect(() => {
    // P4-F: Overview tab has no PTY panes, so silence all forwarding while
    // it's active. Same behaviour as the old `overviewOpen` branch.
    if (activeView === 'overview') {
      sessionSetVisible({ sessionIds: [] }).catch(() => {});
      return;
    }
    if (focusMode !== 'none' && activeSessionId !== null) {
      sessionSetVisible({ sessionIds: [activeSessionId] }).catch(() => {});
      return;
    }
    if (slots.length === 0) {
      sessionSetVisible({ sessionIds: [] }).catch(() => {});
    }
    // When slots.length > 0 and not in overview / focus mode, PaneWorkspace
    // drives `session_set_visible` itself — don't double-call here.
  });

  function handleSelectProject(project: Project): void {
    selectedProjectId = project.id;
    // L4: persist active project selection.
    workspaceStore.update({ active_project_ids: project.id !== null ? [project.id] : [] });
  }

  /** P4-F: setter for the top-level tab selection. Always routes through
   *  this helper so the persistence side-effect can't be forgotten. */
  function setActiveView(v: TabId): void {
    activeView = v;
    workspaceStore.setUi('active_view', v);
  }

  function handleActivateSession(session: SessionSummary): void {
    activeSessionId = session.id;
    // P4-F: activating a session from the alert bar / row click pulls the user
    // out of the Overview tab back onto a pane-rendering tab. Default to
    // Sessions since it's the project-scoped view; the Workspace tab remains
    // a stickier choice that the user must select deliberately.
    if (activeView === 'overview') {
      setActiveView('sessions');
    }
    // T130: persist active session change.
    workspaceStore.update({ focused_session_id: session.id });

    // Phase 4-B/C: activate through the slot set. If the session is already
    // in a slot we leave the layout alone and just re-flash it; otherwise
    // we *replace* the first slot (preserves single-session UX before
    // Phase 4-D adds drag-to-append).
    const existing = slots.findIndex((s) => s.session_id === session.id);
    if (existing === -1) {
      if (slots.length === 0) {
        slots = [{ session_id: session.id, split_percent: 65, order: 0 }];
      } else {
        slots = slots.map((s, i) =>
          i === 0 ? { ...s, session_id: session.id } : s,
        );
      }
      persistSlots();
    }

    // P1-B: Re-trigger the pane border flash. Incrementing the token — rather
    // than setting a boolean — ensures that clicking an already-active row
    // restarts the CSS animation (assigning the same boolean twice would not).
    // The SplitView keys its flash on this token, so a new value = new flash.
    highlightSessionId = session.id;
    highlightToken += 1;
  }

  /** Phase 4-B/C: persist the current slot list into the workspace ui map
   *  so layouts survive restarts. Debounced by workspaceStore. */
  function persistSlots(): void {
    workspaceStore.setUi('workspace_pane_slots', slots);
  }

  /** Phase 5: keep each live slot's `ghost_data` snapshot current. Runs on
   *  every reactive read of `slots`, `sessionsStore.sessions`, and
   *  `projectsStore.projects`. Writes are diff-guarded: only mutates when
   *  the computed snapshot differs from what's stored, so the effect does
   *  NOT self-trigger (no loop). When a live session later transitions to
   *  ended/error or gets pruned, the most-recent snapshot is already on
   *  disk, so the ghost pane has full restart context. */
  $effect(() => {
    // Depend explicitly on the session list so status changes on any
    // tracked slot re-run this effect.
    void sessionsStore.sessions;
    void projectsStore.projects;
    let changed = false;
    const updated = slots.map((slot) => {
      const session = sessionsStore.byId(slot.session_id);
      if (!session) return slot; // ghost stays as-is
      const project = projectsStore.byId(session.project_id) ?? null;
      // Defensively filter non-string command entries (shouldn't happen with
      // a well-behaved backend, but metadata is typed as `Record<string,
      // unknown>` so nothing enforces it at the IPC boundary).
      const rawCommand = session.metadata?.command;
      const command = Array.isArray(rawCommand)
        ? rawCommand.filter((c: unknown): c is string => typeof c === 'string')
        : [];
      const nextGhost: GhostSessionData = {
        project_id: session.project_id,
        label: session.label,
        command,
        project_color: getProjectColor(project),
      };
      const current = slot.ghost_data;
      const diff =
        !current ||
        current.project_id !== nextGhost.project_id ||
        current.label !== nextGhost.label ||
        current.project_color !== nextGhost.project_color ||
        current.command.length !== nextGhost.command.length ||
        current.command.some((c, i) => c !== nextGhost.command[i]);
      if (diff) {
        changed = true;
        return { ...slot, ghost_data: nextGhost };
      }
      return slot;
    });
    if (changed) {
      slots = updated;
      persistSlots();
    }
  });

  /** Phase 5: restart a ghost slot. Called by PaneSlot's ▶ Restart button,
   *  plumbed through PaneWorkspace's `onRestartSlot` prop. Returns the new
   *  session id on success; re-throws on failure so PaneSlot's `handleRestart`
   *  can surface the specific error (e.g. "project archived") rather than a
   *  generic "Failed to restart". */
  async function restartGhostSlot(oldSessionId: number): Promise<number | null> {
    const slot = slots.find((s) => s.session_id === oldSessionId);
    const ghost = slot?.ghost_data;
    if (!slot || !ghost || ghost.command.length === 0) return null;
    const result = await sessionSpawn({
      projectId: ghost.project_id,
      command: ghost.command,
      label: ghost.label,
    });
    const newSession = result.session;
    // Insert into the store immediately so the ghost flips to live without
    // waiting for the `session:spawned` event round-trip. Mirrors the
    // pattern in SpawnSessionDialog's onSpawned handler.
    sessionsStore.add({
      ...newSession,
      activity_summary: null,
      alert: null,
      reattached_mirror: false,
    });
    // Replace only session_id and preserve the full slot object (ghost_data
    // and any future fields) until the snapshot effect refreshes derived data.
    slots = slots.map((s) =>
      s.session_id === oldSessionId
        ? {
            ...s,
            session_id: newSession.id,
          }
        : s,
    );
    persistSlots();
    // Deliberate: we steal activeSessionId for the newly-restarted session.
    // The user just initiated the restart, so landing them in that terminal
    // matches intent — even if a live leftmost slot was active before. The
    // previous activeSessionId was already the ghost that was just replaced
    // in the common case (restart is triggered from the ghost itself).
    activeSessionId = newSession.id;
    highlightSessionId = newSession.id;
    highlightToken += 1;
    workspaceStore.update({ focused_session_id: newSession.id });
    return newSession.id;
  }

  function handleSlotClose(sessionId: number): void {
    const wasActive = sessionId === activeSessionId;
    slots = slots
      .filter((s) => s.session_id !== sessionId)
      .map((s, i) => ({ ...s, order: i }));
    persistSlots();

    if (wasActive) {
      // Closing the active slot drops back to the empty state unless
      // another slot remains, in which case we promote the left-most.
      if (slots.length === 0) {
        activeSessionId = null;
        workspaceStore.update({ focused_session_id: null });
      } else {
        const next = slots[0].session_id;
        activeSessionId = next;
        workspaceStore.update({ focused_session_id: next });
        highlightSessionId = next;
        highlightToken += 1;
      }
    }
  }

  function handleSlotFocus(sessionId: number): void {
    enterFocusMode(sessionId);
  }

  function handleSlotResize(sizes: number[]): void {
    paneSizes = sizes;
    workspaceStore.setUi('workspace_pane_sizes', sizes);
  }

  /** P4-D: append a session to the pane workspace as a new slot. If the
   *  session is already mounted we just re-flash it (no layout change).
   *  This is invoked by:
   *    - the AddSlotZone drop target (drag from SessionList),
   *    - the `⊞` keyboard-accessible button on SessionRow. */
  function addSlot(sessionId: number): void {
    const existingIdx = slots.findIndex((s) => s.session_id === sessionId);
    if (existingIdx === -1) {
      slots = [
        ...slots,
        { session_id: sessionId, split_percent: 65, order: slots.length },
      ];
      persistSlots();
    }
    activeSessionId = sessionId;
    // P4-F: same rationale as handleActivateSession — snap back to a
    // pane-rendering tab when the user pulls a session into a slot from
    // the Overview tab.
    if (activeView === 'overview') {
      setActiveView('sessions');
    }
    workspaceStore.update({ focused_session_id: sessionId });
    highlightSessionId = sessionId;
    highlightToken += 1;
  }

  function onDropSession(sessionId: number): void {
    addSlot(sessionId);
  }

  function onReorderSlots(next: PaneSlot[]): void {
    slots = next;
    persistSlots();
  }

  function onOpenInSlot(session: SessionSummary): void {
    addSlot(session.id);
  }

  /** P4-E: palette activation. If the session is already mounted in a slot
   *  we just re-focus/flash it via the existing single-session path. If it
   *  isn't, we *append* a new slot (addSlot) rather than replace — the
   *  palette is a navigation tool, so losing an already-open pane to swap
   *  in another one would be surprising. `addSlot` itself no-ops when the
   *  session is already mounted (it still updates active + re-flashes), so
   *  the two branches effectively fall through to the same "activate"
   *  behaviour once a slot exists. The branching exists only to avoid
   *  growing the slot set when the session is already there. */
  function handlePaletteActivate(sessionId: number): void {
    const session = sessionsStore.byId(sessionId);
    if (!session) {
      paletteOpen = false;
      return;
    }
    const existing = slots.findIndex((s) => s.session_id === sessionId);
    if (existing !== -1) {
      handleActivateSession(session);
    } else {
      addSlot(sessionId);
    }
    paletteOpen = false;
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

    // P4-E: Ctrl+K (or Cmd+K on mac) opens the quick-switch palette. Guarded
    // by isEditableTarget so typing in a text input doesn't steal the shortcut
    // — but xterm.js also sits inside an editable helper-textarea, so this
    // means the palette cannot be opened while the terminal has focus (users
    // must click outside the terminal first). That's the documented trade-off
    // shared with the `/` filter shortcut above.
    if (
      event.key === 'k' &&
      (event.ctrlKey || event.metaKey) &&
      !event.altKey &&
      !event.shiftKey &&
      !isEditableTarget(event.target)
    ) {
      event.preventDefault();
      paletteOpen = true;
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

  // ─── Sidebar toggle ────────────────────────────────────────────────────
  function toggleSidebar(nextOpen: boolean): void {
    sidebarCollapsed = !nextOpen;
    workspaceStore.setUi('sidebar_collapsed', sidebarCollapsed);
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

      // P3-A / P3-B / P4-F: restore UI state from the persisted ui map.
      if (typeof ws.ui?.sidebar_collapsed === 'boolean') {
        sidebarCollapsed = ws.ui.sidebar_collapsed;
      }
      // P4-F: hydrate the active tab. Guards against older schemas
      // (including the removed `overviewOpen` boolean) by validating the
      // union before assigning.
      const restoredView = ws.ui?.active_view;
      if (
        restoredView === 'sessions' ||
        restoredView === 'workspace' ||
        restoredView === 'overview'
      ) {
        activeView = restoredView;
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
      // a persisted layout had split-two, degrade to single on the first id
      // and also persist the degraded value so subsequent restarts are
      // idempotent (review fix #3).
      if (focusMode === 'split-two') {
        focusMode = focusSessionIds.length > 0 ? 'single' : 'none';
        workspaceStore.setUi('focus_mode', focusMode);
        if (focusMode === 'single' && focusSessionIds.length > 1) {
          focusSessionIds = [focusSessionIds[0]];
          workspaceStore.setUi('focus_mode_session_ids', focusSessionIds);
        }
      }

      // 2. Projects + sessions in parallel.
      await Promise.all([
        projectsStore.hydrate({ includeArchived: true }),
        sessionsStore.hydrate(),
      ]);

      // 3. Phase 4-B/C + Phase 5: rehydrate the pane-workspace slot list.
      //    Phase 4 silently dropped slots whose session_id was no longer in
      //    the store; Phase 5 keeps them and renders them as ghost slots so
      //    the user can restart the stored command. Only slots whose shape
      //    is structurally invalid (missing session_id, etc.) are dropped.
      const rawSlots = ws.ui?.workspace_pane_slots;
      if (Array.isArray(rawSlots)) {
        const kept: PaneSlot[] = [];
        for (const entry of rawSlots) {
          if (
            entry &&
            typeof entry === 'object' &&
            typeof (entry as PaneSlot).session_id === 'number'
          ) {
            const e = entry as PaneSlot;
            const ghost = e.ghost_data;
            const validGhost =
              ghost &&
              typeof ghost === 'object' &&
              typeof ghost.project_id === 'number' &&
              typeof ghost.label === 'string' &&
              Array.isArray(ghost.command) &&
              ghost.command.every((c: unknown) => typeof c === 'string')
                ? {
                    project_id: ghost.project_id,
                    label: ghost.label,
                    command: ghost.command,
                    project_color:
                      typeof ghost.project_color === 'string'
                        ? ghost.project_color
                        : null,
                  }
                : undefined;
            kept.push({
              session_id: e.session_id,
              split_percent: typeof e.split_percent === 'number' ? e.split_percent : 65,
              order: typeof e.order === 'number' ? e.order : kept.length,
              ...(validGhost ? { ghost_data: validGhost } : {}),
            });
          }
        }
        kept.sort((a, b) => a.order - b.order);
        slots = kept.map((s, i) => ({ ...s, order: i }));
      }

      // Fallback: if no stored slots but there IS a focused session,
      // materialise a single slot so the content panel isn't empty.
      if (slots.length === 0 && activeSessionId !== null) {
        slots = [{ session_id: activeSessionId, split_percent: 65, order: 0 }];
      }

      // Promote to active the first slot whose session is actually live
      // (not a ghost). If the only slots are ghosts, leave activeSessionId
      // null so there's no live target for highlight/flash/SessionList. A
      // ghost's Restart button is the user's path back.
      if (slots.length === 0) {
        activeSessionId = null;
      } else {
        const firstLive = slots.find(
          (s) => sessionsStore.byId(s.session_id) !== undefined,
        );
        if (firstLive) {
          if (
            activeSessionId === null ||
            !slots.some(
              (s) =>
                s.session_id === activeSessionId &&
                sessionsStore.byId(s.session_id) !== undefined,
            )
          ) {
            activeSessionId = firstLive.session_id;
          }
        } else {
          activeSessionId = null;
          if (focusMode !== 'none') {
            focusMode = 'none';
            focusSessionIds = [];
            workspaceStore.setUi('focus_mode', focusMode);
            workspaceStore.setUi('focus_mode_session_ids', focusSessionIds);
          }
        }
      }

      const rawSizes = ws.ui?.workspace_pane_sizes;
      if (Array.isArray(rawSizes) && rawSizes.every((v) => typeof v === 'number')) {
        paneSizes = rawSizes as number[];
      }
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

<div
  class="app-layout"
  class:focus-mode={focusMode !== 'none'}
  class:sidebar-collapsed={sidebarCollapsed}
>
  <Sidebar
    {selectedProjectId}
    onSelectProject={handleSelectProject}
    onSpawnSession={(project) => openSpawnDialog(project)}
    open={focusMode === 'none' && !sidebarCollapsed}
    contentId="sidebar-collapsible-content"
    onToggle={toggleSidebar}
  />

  <main class="main-panel">
    <!-- AlertBar lives at the top of .main-panel so it stays visible in
         focus mode. Alerts must never be hidden. -->
    <div class="alert-bar-frame">
      <AlertBar onActivateSession={handleActivateSession} />
    </div>

    <!-- P4-F: Top-level Sessions / Workspace / Overview tabs own the
         entire area below the AlertBar. Each tab's body is a snippet so the
         same state (slots, activeSessionId, focus mode) can be shared and
         only the ONE tab's body renders at a time.

         Sessions vs Workspace differ in a single line: the `filterProjectId`
         passed to SessionList. Sessions uses `selectedProjectId`; Workspace
         forces `null` so the list shows every project. Both share the same
         `slots` array (persisted as `workspace_pane_slots`) — the spec
         hinted at separate `sessions_pane_slots`, but for Phase 4-F we
         collapse to one state to keep the slot set stable when the user
         toggles between tabs.

         Review fix (Phase 4 UX): when focus mode is active, hide MainTabs
         entirely and render the focus-mode body directly. Spec §3.2 intent
         is "reduce peripheral chrome" while zoomed into one session, so the
         tab strip must also disappear (not just the sidebars / session
         panel). The `paneContent` snippet already handles the focus-mode
         branch (renders breadcrumb + × + single SplitView). -->
    {#if focusMode !== 'none'}
      <div class="content-panel">
        {@render paneContent()}
      </div>
    {:else}
      <MainTabs
        value={activeView}
        onValueChange={setActiveView}
        sessionsContent={sessionsTab}
        workspaceContent={workspaceTab}
        overviewContent={overviewTab}
      />
    {/if}
  </main>
</div>

{#snippet sessionsTab()}
  <div class="main-panel-body">
    <div class="session-panel">
      <div class="session-panel-header">
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
        {onOpenInSlot}
        onSpawnSession={() => openSpawnDialog(
          selectedProjectId !== null
            ? projectsStore.byId(selectedProjectId) ?? null
            : null,
        )}
      />
    </div>

    <div class="content-panel">
      {@render paneContent()}
    </div>
  </div>
{/snippet}

{#snippet workspaceTab()}
  <div class="main-panel-body">
    <div class="session-panel">
      <div class="session-panel-header">
        <!-- P4-F: LayoutSwitcher lives in the Workspace tab header because
             saved layouts capture the pane arrangement, which is the
             central concept of this tab. The wrapping `div` with
             `margin-right: auto` keeps the Switcher flush-left while
             Settings stays pinned to the right — the header itself is
             `justify-content: flex-end` for the Sessions tab case where
             only Settings renders. -->
        <div class="layout-slot">
          <LayoutSwitcher onMissingSessions={(ids) => { missingSessions = new Set(ids); }} />
        </div>
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
        selectedProjectId={null}
        {missingSessions}
        {activeSessionIds}
        onActivateSession={handleActivateSession}
        {onOpenInSlot}
        onSpawnSession={() => openSpawnDialog(
          selectedProjectId !== null
            ? projectsStore.byId(selectedProjectId) ?? null
            : null,
        )}
      />
    </div>

    <div class="content-panel">
      {@render paneContent()}
    </div>
  </div>
{/snippet}

{#snippet overviewTab()}
  <div class="overview-tab-body">
    <CrossProjectOverview />
  </div>
{/snippet}

{#snippet paneContent()}
  {#if focusMode !== 'none' && activeSession && activeSessionId !== null}
    <!-- P3-B focus mode: single-session override that takes precedence
         over the multi-pane workspace. Keeps the compact breadcrumb
         chip + exit button since the user has explicitly zoomed in on
         one session. The SplitView is rendered directly (not through
         PaneWorkspace) to avoid the pane header chrome stacking. -->
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
  {:else if slots.length > 0}
    <!-- Phase 4-B/C: multi-pane workspace. Replaces the single-
         SplitView render path. The old `.active-session-header` now
         lives inside `PaneSlot` so every visible slot carries its own
         project tint + controls. The `{#key}` on session id +
         reattached_mirror is preserved inside PaneSlot via
         SplitView's own mount flow. -->
    <PaneWorkspace
      {slots}
      {paneSizes}
      highlightedSessionId={highlightSessionId}
      {highlightToken}
      onSlotClose={handleSlotClose}
      onSlotFocus={handleSlotFocus}
      onResize={handleSlotResize}
      {onDropSession}
      {onReorderSlots}
      onRestartSlot={restartGhostSlot}
    />
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
{/snippet}

<SettingsDialog open={settingsOpen} onclose={() => settingsOpen = false} />

<CommandPalette
  open={paletteOpen}
  onClose={() => (paletteOpen = false)}
  onActivate={handlePaletteActivate}
/>

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
    // P4-F: ensure we're on a pane-rendering tab so the new session is
    // actually visible after spawn.
    if (activeView === 'overview') {
      setActiveView('sessions');
    }
    workspaceStore.update({ focused_session_id: session.id });
    // Phase 4-B/C: also seed the slot set so PaneWorkspace renders the new
    // session. If slots is empty → create the first one; otherwise replace
    // the first slot (matches handleActivateSession's single-session UX).
    if (slots.length === 0) {
      slots = [{ session_id: session.id, split_percent: 65, order: 0 }];
    } else if (!slots.some((s) => s.session_id === session.id)) {
      slots = slots.map((s, i) => (i === 0 ? { ...s, session_id: session.id } : s));
    }
    persistSlots();
    highlightSessionId = session.id;
    highlightToken += 1;
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

  /* Hamburger button and hover-peek hotzone removed — sidebar edge toggle
     handles collapse/expand directly. */

  /* Focus mode hides the sidebar wrapper entirely so the content area
     expands to full width. The sidebar's `open` prop is already set to
     false when focus mode is active. */
  .app-layout.focus-mode :global(.sidebar-wrapper) {
    display: none;
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

  /* P4-F: the old `.overview-btn` rules were removed — Overview is now a
     top-level tab (see MainTabs.svelte). The Workspace tab wraps its
     LayoutSwitcher in a `.layout-slot` div with `margin-right: auto` so
     the Switcher sticks to the left edge of the header while Settings
     stays on the right (inherits `justify-content: flex-end` from the
     .session-panel-header flex row). */
  .layout-slot {
    margin-right: auto;
    display: flex;
    align-items: center;
  }

  .overview-tab-body {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: flex-start;
    overflow: hidden;
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

  /* Phase 4-B/C: the old `.active-session-header` + `.focus-enter-btn` +
     `.session-status` selectors lived here to style the single-session
     header that has now moved into `PaneSlot.svelte`. Focus mode still
     uses `.split-view-wrapper` (below) and `.readonly-banner` for its
     breadcrumb chip — the rest were pruned to satisfy svelte-check's
     unused-selector warnings. */

  /* Layout carrier for the SplitView in focus mode: `flex: 1;
     min-height: 0; overflow: hidden;` give the split its bounding box
     inside the content panel. Threads `--project-color` via the inline
     `style` attribute so the Phase 1 flash overlay picks up the same
     per-project colour. */
  .split-view-wrapper {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
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
