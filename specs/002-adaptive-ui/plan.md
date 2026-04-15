# Feature 002 — Adaptive UI: Implementation Plan

**Feature**: `002-adaptive-ui`
**Date**: 2026-04-15
**Status**: Draft
**Depends on**: `spec.md`, `research.md`

---

## Principles

1. **Adopt libraries before writing custom code.** Every phase lists which external packages to install before touching components.
2. **Each phase ships independently.** Phases 1–3 have no dependency on later phases and can be reviewed + merged alone.
3. **Backend changes are minimal and backward-compatible.** The `settings_json` and `workspace_state.payload_json` fields are already freeform; no SQL migrations until Phase 4.
4. **One new component per new concept.** Don't extend existing components to do two jobs; add a new wrapper and keep the original intact.

---

## Phase 1 — Quick Wins (no new libraries)

These changes touch only existing components with no architectural shifts. Estimated: small, high-confidence.

### P1-A: Active Session Indicator in SessionList

**Files**: `SessionRow.svelte`, `SessionList.svelte`, `+page.svelte`

1. Add `active: boolean` prop to `SessionRow`. Apply `.session-row.active` CSS (left-border 2 px in `--project-color`, dot `::before`, background tint via `color-mix`).
2. Add `activeSessionIds: Set<number>` prop to `SessionList`. Pass down to each `SessionRow`.
3. In `+page.svelte`, derive `activeSessionIds` from the current `activeSessionId` (initially a `Set` of one; becomes a `Set` of the visible slot IDs in Phase 4).
4. Add `--project-color` CSS variable prop to `SessionRow` and `SessionList.group-heading`. Read from `projectsStore.byId(session.project_id)?.settings.color ?? '--color-accent'`.
5. Dim non-active `.session-main` text to 70% opacity when any session is active (CSS: `.session-list:has(.session-row.active) .session-row:not(.active) .session-main { opacity: 0.7; }`).

### P1-B: AlertBar "Go to" Flash and Scroll

**Files**: `AlertBar.svelte`, `+page.svelte`, `SplitView.svelte`

1. Add `highlighted: boolean` prop to `SplitView`. When true, add `.split-view-highlighted` class for a 1.5 s border-flash CSS animation (`@keyframes flash: border-color project-color → transparent`).
2. In `+page.svelte`: after `handleActivateSession` fires, set a `highlightSessionId` state for 1500 ms, then clear it. Pass `highlighted={highlightSessionId === activeSessionId}` to `SplitView`.
3. In `AlertBar.svelte`: after calling `onActivateSession`, emit a custom DOM event `session-scroll-to` with the session ID. `SessionList` listens and calls `document.querySelector(`[data-session-id="${id}"]`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })`.
4. Add `data-session-id={session.id}` attribute to `SessionRow` root element.

### P1-C: Filter Focus Shortcut

**Files**: `+page.svelte`, `SessionList.svelte`

1. Add a `focusFilter()` method exported from `SessionList` (or pass a ref to the filter input).
2. In `+page.svelte`, register a `keydown` listener: if `e.key === '/' && !e.ctrlKey && !e.metaKey && !isTextInput(e.target)`, call `focusFilter()`.
3. Guard: `isTextInput(target)` returns true if the target is an `<input>`, `<textarea>`, or `[contenteditable]` — prevents triggering when the user is typing in xterm.

### P1-D: Refresh Button Spinners

**Files**: `SpinnerIcon.svelte` (new), `CrossProjectOverview.svelte`, `LayoutSwitcher.svelte`

1. Create `SpinnerIcon.svelte`: a 14 px rotating arc (pure CSS, no SVG file dependency).
2. In `CrossProjectOverview.svelte`: replace the Refresh button label with `{#if loading}<SpinnerIcon />{:else}Refresh{/if}`. Disable button while loading. (The `overviewStore.loading` state already exists.)
3. In `LayoutSwitcher.svelte`: show `<SpinnerIcon />` in the dropdown header while the `refresh()` async call is in-flight. Add a `refreshing` local boolean state.

### P1-E: AlertBar Dark-Theme Warning Colour

**Files**: `AlertBar.svelte`, `app.css`

1. In `app.css`, define `--color-warning-bg: #3d2e00` and `--color-warning: #fbbf24` (dark-compatible, maintains salience on dark theme without switching to light-mode palette).
2. `AlertBar.svelte` already uses these variables; the visual change is automatic once the values are updated.

---

## Phase 2 — Project Colour Coding

**New package**: `vanilla-colorful` (2.7 KB, zero deps).

### P2-A: Backend — Auto-assign colour on project_register

**Files**: `src-tauri/src/commands/projects.rs`, `src-tauri/src/model/project.rs`

1. Add `color: Option<String>` to `ProjectSettings` Rust struct with `#[serde(skip_serializing_if = "Option::is_none")]`.
2. In `project_register`: after inserting the row, read `SELECT COUNT(*) FROM projects WHERE archived_at IS NULL` to get the project index. Set `settings.color = PALETTE[count % 12]` where `PALETTE` is the 12-colour array from `spec.md §1.2`. Update the row.
3. TypeScript `ProjectSettings` interface: add `color?: string`.

### P2-B: Sidebar — Colour swatch and picker

**Files**: `Sidebar.svelte`, new `ColorSwatchPicker.svelte`

1. Add `--project-color: {project.settings.color ?? 'var(--color-accent)'}` inline style to each `.project-item`.
2. Change the left-border accent from a hardcoded `--color-accent` to `--project-color` for `.project-item.selected`.
3. On hover of `.project-item`, reveal a small colour swatch button (8 × 8 px filled circle, `--project-color` background).
4. Clicking the swatch opens `ColorSwatchPicker.svelte`: wraps `<hex-color-picker>` from `vanilla-colorful` in a `bits-ui Dialog` (or CSS `popover` if bits-ui is not yet installed — add it in Phase 5). On colour selection, call `projectsStore.update(id, { settings: { ...settings, color: hex } })`.

### P2-C: Session List — Colour propagation

**Files**: `SessionList.svelte`, `SessionRow.svelte`

1. Look up `projectsStore.byId(session.project_id)?.settings?.color` and pass as `--project-color` to `SessionRow` (already wired in P1-A, this phase adds the actual colour value).
2. Apply the same colour to `.group-heading` left-border.

### P2-D: Pane Header — Colour tinting

**Files**: `+page.svelte`, `SplitView.svelte` (or the new `PaneSlot.svelte` from Phase 4)

1. Pass the active session's project colour as `--project-color` to the `active-session-header` div.
2. Apply `border-left: 4px solid var(--project-color)` and `background: color-mix(in srgb, var(--project-color) 8%, var(--color-surface))` to the header.
3. The companion pane header (inside `CompanionPane.svelte`) inherits the colour via the cascade if `--project-color` is set on the `SplitView` wrapper.

---

## Phase 3 — Collapsible Sidebar + Focus Mode

**New package**: `bits-ui` (for Collapsible, and pre-staging for Phase 5 Tabs + Dialog).

### P3-A: Sidebar Collapse

**Files**: `+page.svelte`, `Sidebar.svelte`, new `HamburgerButton.svelte`

1. Install `bits-ui`. Wrap `<Sidebar>` in a `<Collapsible.Root>` with `open={!sidebarCollapsed}`.
2. The `<Collapsible.Content>` wraps the full sidebar body. Add CSS transition:
   ```css
   [data-state=closed] { width: 0; overflow: hidden; }
   [data-state=open] { width: 260px; }
   .sidebar-collapsible { transition: width 200ms ease; }
   ```
3. `HamburgerButton.svelte`: a `<Collapsible.Trigger>` rendered as a `☰` button, always visible in the `.main-panel` top-left corner. Also serves as the hover-peek trigger.
4. Hover peek: when the sidebar is collapsed, hovering the hamburger area (48 px strip) adds class `.sidebar-peeking` which sets `position: absolute; z-index: 50; width: 260px` — the sidebar overlays the content without compressing it.
5. Persist `sidebarCollapsed` in `workspaceStore.ui.sidebar_collapsed`.

### P3-B: Focus Mode

**Files**: `+page.svelte`, `SplitView.svelte`

1. Add `focusMode: 'none' | 'single' | 'split-two'` state to `+page.svelte`.
2. Add `focusSessionIds: number[]` state.
3. In focus mode, apply CSS class `.app-layout.focus-mode` which hides `.sidebar` and `.session-panel` with a slide-out transition (200 ms), and expands `.content-panel` to full width.
4. `AlertBar` is rendered outside the `.session-panel` at the `.main-panel` level so it is not hidden by focus mode.
5. Focus mode controls:
   - ⊙ button added to `active-session-header` → enters `single` focus.
   - Keyboard `Ctrl+Shift+F` → toggle `single` focus.
   - `×` button always-visible in top-right corner of `.content-panel` → exits focus mode.
   - Identity chip (`● project / label — status`) shown in the content header area when `focusMode !== 'none'`.
6. Persist `focus_mode` + `focus_mode_session_ids` in `workspaceStore.ui`.

---

## Phase 4 — Multi-Pane Workspace

**New packages**: `paneforge`, `svelte-dnd-action`.

This is the largest phase. It introduces a new layout model for the content area.

### P4-A: Backend — session_set_visible

**Files**: `src-tauri/src/commands/sessions.rs`, `src-tauri/src/session/focus.rs` (or equivalent)

1. Add `session_set_visible(session_ids: Vec<i64>)` Tauri command.
2. The implementation replaces the single `focused_session_id` with a `HashSet<SessionId>` of visible sessions.
3. PTY output forwarding logic: forward `session:event` / `companion:output` events for any session whose ID is in the visible set.
4. Keep `session_set_focus(session_id: i64 | null)` as a backward-compatible shim: `session_set_visible([session_id])` or `session_set_visible([])`.

### P4-B: PaneWorkspace.svelte (new component)

Replaces the current single `SplitView` in the content area.

Structure:
```svelte
<PaneGroup direction="horizontal" class="pane-workspace">
  {#each slots as slot, i (slot.id)}
    <Pane minSize={520} defaultSize={100 / slots.length}>
      <PaneSlot
        sessionId={slot.session_id}
        projectColor={getProjectColor(slot.session_id)}
        onClose={() => removeSlot(slot.id)}
        onFocus={() => enterFocusMode(slot.session_id)}
        highlighted={highlightSessionId === slot.session_id}
      />
    </Pane>
    {#if i < slots.length - 1}
      <PaneResizer class="pane-resizer-vertical" />
    {/if}
  {/each}
  {#if slots.length < MAX_SLOTS}
    <AddSlotZone {onDrop} />
  {/if}
</PaneGroup>
```

- `slots` is derived from `workspaceStore.ui.workspace_pane_slots` (or `sessions_pane_slots` depending on active tab).
- Pane sizes are persisted back to the store via paneforge's `onResize` callback.
- `MAX_SLOTS` is computed from `Math.floor(contentWidth / MIN_PANE_WIDTH)` where `MIN_PANE_WIDTH = 520`.

### P4-C: PaneSlot.svelte (new component)

Wraps a single pane slot. Contains:
- Pane header (project colour strip, label, status, close/focus buttons, drag handle)
- `SplitView` (existing, unchanged) for the session itself
- Ghost state (when session is ended/pruned — shows command + Restart button)

The drag handle on the pane header is a `use:draggable` source for `svelte-dnd-action` (slot reordering).

### P4-D: Session list as DnD source

**Files**: `SessionList.svelte`, `SessionRow.svelte`

1. Wrap the `session-list-body` in a `use:dndzone` from `svelte-dnd-action` configured as a source-only zone (items can be dragged out but the list itself doesn't reorder).
2. The `AddSlotZone` component in `PaneWorkspace` is the drop target.
3. Drag ghost: while dragging a session row, show the session label + project colour dot as the drag preview.
4. Additionally, add a small ⊞ button to each `SessionRow` that calls `addSlot(session)` directly without dragging.

### P4-E: Quick-Switch Palette

**Files**: new `CommandPalette.svelte`

1. A modal overlay (full-screen semi-transparent backdrop) that opens on `Ctrl+K`.
2. Auto-focused text input with fuzzy filter (reuses `matchesSessionFilter`).
3. Session list grouped by project, showing: colour dot, label, status badge, alert badge.
4. `ArrowUp` / `ArrowDown` navigation, `Enter` to activate, `Escape` to dismiss.
5. On activation: if the session is already in a slot, calls `sessionSetFocus` on that slot; otherwise calls `addSlot(session)`.
6. Session count + alert-count badges on project group headers within the palette.
7. Registered in `+page.svelte` global keydown handler.

### P4-F: Multi-View Tabs

**Files**: `+page.svelte`, new `MainTabs.svelte`

1. Use `bits-ui` `Tabs.Root` / `Tabs.List` / `Tabs.Trigger` / `Tabs.Content`.
2. Three tabs: Sessions, Workspace, Overview.
3. Tab bar replaces the current `.session-panel-header` button row.
4. Sessions tab: session panel (left) + sessions-scoped pane workspace (right).
5. Workspace tab: full-width session list (all projects) + workspace-scoped pane workspace.
6. Overview tab: `CrossProjectOverview.svelte` at full width.
7. Persist `active_view` in `workspaceStore.ui`.
8. Move `LayoutSwitcher` to the Workspace tab header.

### P4-G: Overflow indicator

**Files**: `PaneWorkspace.svelte`

1. Track which slots are within the visible width vs. hidden (computed from container width + `MIN_PANE_WIDTH`).
2. Show `[+N more]` badge at the right edge of the pane header strip when slots overflow.
3. Clicking the badge opens a compact list of hidden sessions (small popover). Clicking a session swaps it with the rightmost visible slot.

---

## Phase 5 — Session Ghost Restore

**No new packages** (paneforge + bits-ui + svelte-dnd-action already installed).

### P5-A: Ghost slot detection

**Files**: `+page.svelte`, `PaneWorkspace.svelte`, `PaneSlot.svelte`

1. On `workspaceStore.hydrate()`, after `sessionsStore.hydrate()`, iterate `pane_slots` and classify each as `live` (session still running) or `ghost` (session ID not in store, or status is `ended`/`error`).
2. Ghost slots are rendered by `PaneSlot` in a special ghost state (see P4-C).
3. Ghost data needed: project ID, session label, command (from `metadata.command[]`). This is stored in `workspaceStore.ui.pane_slots` alongside `session_id` — add `ghost_data?: GhostSessionData` to `PaneSlot` interface:
   ```typescript
   interface PaneSlot {
     session_id: number;
     split_percent: number;
     order: number;
     // Populated from the last live session snapshot, persisted for ghost restore:
     ghost_data?: {
       project_id: number;
       label: string;
       command: string[];
       project_color: string;
     };
   }
   ```
4. When a slot becomes live, overwrite `ghost_data` with the current session's data (kept fresh so if the session ends later, the ghost has current info).

### P5-B: Ghost slot UI

**Files**: `PaneSlot.svelte`

Ghost pane shows:
- Project colour strip (from `ghost_data.project_color`)
- Session label (greyed out, 60% opacity)
- "Ended" badge
- Command block: `<code>{ghost_data.command.join(' ')}</code>`
- **▶ Restart** button: calls `sessionSpawn({ projectId, command, label })` → on success, updates the slot's `session_id` to the new session ID and clears ghost state.
- **✕ Remove** button: removes the slot entirely.

### P5-C: Auto-save ghost_data

**Files**: `PaneWorkspace.svelte`

1. Watch `sessionsStore` for status changes. When a slot's session transitions to `ended`/`error`, immediately snapshot `ghost_data` into the workspace store (before the session row is pruned from the store).
2. The `session:ended` event handler in `sessionsStore.svelte.ts` emits a local notification; `PaneWorkspace` subscribes and updates the slot.

---

## Dependency Installation

All packages are installed at the start of their phase:

```bash
# Phase 2
pnpm add vanilla-colorful

# Phase 3
pnpm add bits-ui

# Phase 4
pnpm add paneforge svelte-dnd-action
```

No new Rust crates are required — Phase 4-A extends existing session focus logic only.

---

## File Change Summary

| File | Phase | Change type |
|---|---|---|
| `src/app.css` | P1-E, P2 | Modify: palette vars, warning colours |
| `src/lib/components/SessionRow.svelte` | P1-A, P2-C | Modify: `active` prop, `--project-color` prop |
| `src/lib/components/SessionList.svelte` | P1-A, P1-C, P2-C, P4-D | Modify: pass activeSessionIds, DnD source |
| `src/lib/components/SplitView.svelte` | P1-B, P2-D, P3-B | Modify: `highlighted` prop, focus mode support |
| `src/lib/components/AlertBar.svelte` | P1-B, P1-E | Modify: scroll-emit, colour fix |
| `src/lib/components/Sidebar.svelte` | P2-B, P3-A | Modify: colour swatch, collapse wrapper |
| `src/lib/components/CrossProjectOverview.svelte` | P1-D | Modify: spinner |
| `src/lib/components/LayoutSwitcher.svelte` | P1-D | Modify: spinner |
| `src/lib/stores/workspace.svelte.ts` | P3, P4, P5 | Modify: ui type, slot management helpers |
| `src/routes/+page.svelte` | All phases | Modify: orchestration, new state |
| `src/lib/components/SpinnerIcon.svelte` | P1-D | **New** |
| `src/lib/components/ColorSwatchPicker.svelte` | P2-B | **New** |
| `src/lib/components/PaneWorkspace.svelte` | P4-B | **New** |
| `src/lib/components/PaneSlot.svelte` | P4-C, P5-B | **New** |
| `src/lib/components/CommandPalette.svelte` | P4-E | **New** |
| `src/lib/components/MainTabs.svelte` | P4-F | **New** |
| `src/lib/components/AddSlotZone.svelte` | P4-D | **New** |
| `src/lib/api/sessions.ts` | P4-A | Modify: add `sessionSetVisible` |
| `src-tauri/src/model/project.rs` | P2-A | Modify: `color` field in ProjectSettings |
| `src-tauri/src/commands/projects.rs` | P2-A | Modify: colour auto-assign in register |
| `src-tauri/src/commands/sessions.rs` | P4-A | Modify: `session_set_visible` command |
| `src-tauri/src/session/focus.rs` (or equivalent) | P4-A | Modify: multi-session focus set |

---

## Risk Register

| Risk | Likelihood | Mitigation |
|---|---|---|
| `paneforge` pixel-based `minSize` clashes with its percent-based API | Medium | paneforge uses pixels for `minSize` in v1.0; verify with a prototype before Phase 4 commits |
| `svelte-dnd-action` compat mode causes subtle reactivity bugs in Svelte 5 runes context | Low–Medium | Use `@thisux/sveltednd` as drop-in fallback (it is runes-native); API is similar enough to swap |
| xterm.js `FitAddon` does not re-fit when pane width changes via paneforge | Medium | Call `fitAddon.fit()` in a `ResizeObserver` on the terminal container (already likely needed); paneforge's `onResize` callback is the trigger |
| Ghost-slot `ghost_data` gets stale if a session is pruned before workbench saves | Low | Phase 5-C saves `ghost_data` on `session:ended` event, before pruning |
| `color-mix()` not supported in older Tauri WebView | Low | Tauri 2 uses a modern Chromium/WebKit WebView; `color-mix()` is supported. Provide `background: rgba(...)` fallback via PostCSS `@supports` if needed |
| `bits-ui Collapsible` animate-height conflicts with fixed-width sidebar | Low | Use width transition, not height; bits-ui `forceMount` keeps DOM node present for CSS transitions |

---

## Testing Notes

- Phase 1 changes are covered by existing vitest/Playwright setup; add regression tests for SessionRow active state.
- Phase 4 requires Playwright tests for: DnD slot creation, palette navigation, slot overflow, ghost slot restart.
- Phase 4-A backend change requires a new Rust unit test: `session_set_visible([1, 2])` → output events for sessions 1 and 2 are forwarded; output for session 3 is not.
- Minimum pane width enforcement should have a Playwright test: resize window to very narrow → verify slots hide rather than compress.
