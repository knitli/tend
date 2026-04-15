# Feature 002 — Adaptive UI

**Feature**: `002-adaptive-ui`
**Date**: 2026-04-15
**Status**: Draft
**Depends on**: `research.md`
**Scope**: Frontend (Svelte 5) with minor Rust/Tauri backend extensions

---

## 0. Design Philosophy

This spec puts **user experience ahead of individual feature requirements**. Where an external, well-maintained library covers 60–90% of a desired behaviour, we adopt it — someone else absorbed the bug-hunting cost. Custom components are written only when no library fit exists.

All features in §§1–7 share a common design language:

- **Project colour** is the single strongest identity signal. Every surface that touches a project — sidebar button, session row, pane header, status bar tint — carries its colour.
- **Keyboard first, mouse also works.** Every interaction has both a keyboard path and a click path.
- **Persistence without surprise.** UI state is saved on every change (debounced 250 ms, existing pattern). On restart the workbench re-opens exactly as it was left, with ended sessions shown as restartable ghosts rather than silently dropped.
- **Alerts never get hidden.** Focus mode, collapsed sidebar, and multi-pane layouts all preserve the AlertBar.

---

## 1. Project Colour Coding

### 1.1 Goal

Each project gets a stable colour. That colour propagates to every UI surface that represents the project:

| Surface | Colour application |
|---|---|
| Sidebar project button | Left-border accent + subtle background tint |
| Session list — group heading | Left-border accent; alert count badge uses colour |
| Session list — session rows | Left-border dot indicator (2 px) |
| Active session pane header | Left-border strip (4 px) + header background tint (8–12% opacity) |
| CompanionPane header / restart button | Same tint |
| Status bar (future) | Background tint |

The tint is always the project colour at low opacity (≈ 10%) to keep the dark theme readable. The full colour is used only for accent borders and indicators.

### 1.2 Auto-assignment

A 12-colour palette is defined in `app.css` as `--palette-*` variables. Colours are chosen to be:
- Perceptually distinct from each other
- Readable as left-border accents on the dark background (`#0f1115`)
- Reasonably distinguishable under protanopia / deuteranopia (avoid red/green pairs as the sole distinguishing factor)

Suggested palette (12 colours):
```
#60a5fa  blue       (current accent — assign to first project)
#a78bfa  violet
#34d399  emerald
#fb923c  orange
#f472b6  pink
#38bdf8  sky
#facc15  yellow
#4ade80  green
#c084fc  purple
#f87171  red
#2dd4bf  teal
#e879f9  fuchsia
```

On `project_register`, the backend assigns `settings.color` as `palette[(project_count % 12)]`. Since `settings_json` is already a freeform column, **no schema migration is required**.

The frontend reads `project.settings.color` wherever a project colour is needed. A fallback value (`--color-accent`) is used when `color` is absent (existing projects before this feature).

### 1.3 User Customisation

A colour swatch button appears on hover of the project item in the Sidebar. Clicking it opens a `<hex-color-picker>` web component from `vanilla-colorful` (2.7 KB, framework-agnostic, no Tailwind). Selecting a colour calls `projectUpdate({ id, settings: { ...current.settings, color: hex } })` immediately.

The picker appears in a small `bits-ui Dialog` (or a CSS popover, whichever is simpler) anchored to the swatch button. Pressing Escape or clicking outside dismisses it.

### 1.4 API / Backend changes

- `ProjectSettings` TypeScript interface: add optional `color?: string` field.
- Rust `ProjectSettings` struct: add `#[serde(skip_serializing_if = "Option::is_none")] pub color: Option<String>`.
- `project_register` Tauri command: after inserting the row, read the count of active projects and set `settings.color` from the palette if no colour was supplied.
- `project_update` Tauri command: already accepts freeform `settings`; no change needed.

No SQL migration required.

---

## 2. Multi-Pane Session Workspace

### 2.1 Goal

Replace the single-session content area with a horizontally-split multi-session workspace. Any session from any project can be placed in any pane slot. Pane slots are resizable and reorderable.

### 2.2 Layout Model

```
┌─────────────────────────────────────────────────────────┐
│  Content Area (PaneWorkspace.svelte)                    │
│  ┌──────────┬──────────┬──────────┐                     │
│  │ Pane A   │ Pane B   │ Pane C   │                     │
│  │ (project │ (project │ (project │                     │
│  │  alpha)  │  alpha)  │  beta)   │                     │
│  │          │          │          │                     │
│  │ [Agent]  │ [Agent]  │ [Agent]  │                     │
│  │ ──────── │ ──────── │ ──────── │                     │
│  │ [Compan] │ [Compan] │ [Compan] │                     │
│  └──────────┴──────────┴──────────┘                     │
└─────────────────────────────────────────────────────────┘
```

- Each pane slot contains one full `SplitView` (agent + companion, existing component unchanged).
- The horizontal dividers between pane slots are provided by `paneforge` (`PaneGroup` / `Pane` / `PaneResizer`).
- Pane order is maintained in `workspaceStore.ui.pane_slots` (see §6).

### 2.3 Pane Sizing Constraints

Minimum pane width per slot: **520 px**.

Rationale: Claude Code in "standard terminal" mode uses 80 columns. At the default monospace font size (13 px), 80 columns ≈ 520 px. Below this width, Claude's output wraps and becomes unreadable. The `paneforge` `minSize` prop is set in pixels (not percent) to enforce this regardless of window width.

If the available content width cannot fit all pane slots at minimum width, the excess slots are hidden behind an overflow control (see §2.5). The workbench does **not** squeeze panes below the minimum — it hides them instead.

Companion terminal minimum height: 25% of the `SplitView` height. The existing `SPLIT_MIN = 20` in `SplitView.svelte` covers this; no change needed.

### 2.4 Drag-and-Drop Session Assignment

Sessions are added to pane slots by:

1. **Drag from session list**: Session rows in `SessionList.svelte` become drag sources using `svelte-dnd-action`. The pane workspace has a set of drop zones (one per slot, plus an empty "add slot" zone at the right edge). Dragging a session row onto a slot replaces that slot's session. Dragging onto the empty zone creates a new slot.

2. **Drag to reorder slots**: Pane slot headers (showing `project / session-label`) are themselves drag sources. Dragging a slot header to a new position in the header row reorders the slots using `svelte-dnd-action`.

3. **Click "Open in pane" from session row context menu** (secondary method): A small ⊞ button on the session row adds the session to a new pane slot without drag.

4. **Double-click session row**: Opens the session in the currently focused pane slot (replaces it). This is the zero-friction path for keyboard users who don't want to drag.

### 2.5 Overflow Handling

When the content width cannot fit all pane slots at minimum width:

- Slots that overflow are **hidden** but remain in `pane_slots` state.
- A **slot overflow indicator** appears at the right edge of the pane header strip: `[+N more]` button. Clicking opens a compact list of hidden sessions; clicking one swaps it with the rightmost visible slot.
- A **quick-switch palette** (`Ctrl+K`) shows all sessions in all slots (visible and hidden) with fuzzy search. Activating a session from the palette brings it into a visible slot (replacing the least-recently-used slot if all slots are occupied). See §2.6.

### 2.6 Quick-Switch Palette

A `CommandPalette.svelte` overlay triggered by `Ctrl+K` (global keyboard shortcut):

- Full-screen modal backdrop
- Search input (auto-focused on open)
- All sessions listed, grouped by project, fuzzy-filtered by label + project name (reuses existing `matchesSessionFilter`)
- Keyboard navigation: `ArrowUp` / `ArrowDown` to move focus, `Enter` to activate, `Escape` to close
- Session rows show: project colour dot, session label, status badge, alert badge if present
- Selecting a session: if it is already in a visible slot, focuses that slot; otherwise opens it in a new or LRU-replacement slot

### 2.7 Default Layout: "Fit All Project Sessions"

When a project is selected in the sidebar (or on first open), the default behaviour is:
1. Calculate how many of the project's active sessions fit at minimum slot width.
2. Open that many slots side-by-side.
3. If more sessions exist than fit, excess sessions are visible in the overflow indicator.

This replaces the current "activate one session at a time" model and makes "see all sessions for this project" the default action.

### 2.8 Visual Identification

Each pane slot header shows:
- **Project colour swatch** (8 px × 8 px filled circle)
- **Project name / Session label** text
- **Status badge** (working / idle / needs_input / ended)
- **Close slot** button (×)
- **Maximise** button (⊙ → focus mode, see §3)
- **Drag handle** (⠿) for slot reordering

The project colour is applied as a left-border strip (4 px) on the pane header and as a 10% opacity background tint on the pane header bar only (not the terminal content).

### 2.9 Backend Change: Multi-Focus

The backend currently supports `session_set_focus(session_id: i64 | null)` which forwards PTY output events for exactly one session. Multi-pane requires forwarding for all visible sessions simultaneously.

**New command**: `session_set_visible(session_ids: Vec<i64>)` — replaces `session_set_focus` for the multi-session case. The backend forwards `session:event` and `companion:output` events for all IDs in the list. The existing per-session filtering in `AgentPane.svelte` and `CompanionPane.svelte` (which already filter by `session_id`) continues to work correctly.

The old `session_set_focus` command can remain as a shim calling `session_set_visible([id])`.

---

## 3. Focus Mode

### 3.1 Three Modes

| Mode | Description |
|---|---|
| `none` | Normal multi-pane workspace |
| `single` | One session fills the entire content area; sidebar and session panel are hidden |
| `split-two` | Two sessions fill the content area side by side; sidebar and session panel are hidden |

Focus mode is entered from:
- The ⊙ button in any pane slot header → `single` focus on that slot
- The ⊙⊙ button (if two slots are selected) → `split-two`
- Keyboard shortcut `Ctrl+Shift+F` → `single` for the most recently active slot
- From the session list: clicking a session and pressing `F` key → `single` focus

### 3.2 Behaviour in Focus Mode

- The `.sidebar` (projects, 260 px) is hidden with a CSS slide-out transition.
- The `.session-panel` (session list, 320 px) is hidden with a CSS slide-out transition.
- `AlertBar` **remains visible** at the top of the content area (alerts must never be hidden — NNGroup finding §B.5 of research.md).
- A compact identity chip is shown in the content area header: `● ProjectName / session-label — Status`. The dot is the project colour.
- An always-visible `×` exit button sits in the top-right corner of the content area (visible even when xterm has keyboard focus — critical because `Escape` may be consumed by the terminal).
- `Escape` in the Svelte layer (not the terminal) also exits focus mode.

### 3.3 Opening a Session from the Session List in Focus Mode

If the user is in focus mode and clicks a session row in the session list (which is hidden), the click opens the session in focus mode replacing the current single session. This is consistent with VS Code's "open file while in Zen Mode closes the old file" behaviour.

### 3.4 Persistence

`focus_mode: 'none' | 'single' | 'split-two'` and `focus_mode_session_ids: number[]` stored in `workspaceStore.ui`.

---

## 4. Refresh / Reload Button Feedback

### 4.1 Problem

The "Overview" and "Layouts" buttons and the `CrossProjectOverview` refresh button trigger async operations but show no loading indicator. Users cannot tell if a click registered.

### 4.2 Solution

A reusable `SpinnerIcon.svelte` component: a small (14 px) CSS-animation spinner (rotating arc, no image file). Used inline next to button labels.

Pattern applied uniformly:
- `CrossProjectOverview.svelte` Refresh button: show spinner while `overviewStore.loading` is true.
- `LayoutSwitcher.svelte` when open (already has a `refresh()` call on open): show a spinner in the dropdown header while `layouts` is being fetched.
- Any future async button follows the same pattern: `{#if loading}<SpinnerIcon />{/if}` inline with the button label.

The spinner replaces the label text while loading (not appended), to avoid layout shift. Button is `disabled` while loading to prevent double-submission.

---

## 5. Multi-View Tabs

### 5.1 Three Views

The main panel (everything to the right of the sidebar) is divided into three views using `bits-ui` Tabs:

| Tab | Key | Content |
|---|---|---|
| **Sessions** | `sessions` | Project-filtered session list + pane workspace for the selected project |
| **Workspace** | `workspace` | Full cross-project multi-pane workspace with drag-and-drop |
| **Overview** | `overview` | Cross-project reminders (`CrossProjectOverview.svelte`, replaces current "Overview" button) |

The tab bar sits at the very top of the `.main-panel`. The `Sidebar` (project list) is **outside** the tab structure and always visible.

### 5.2 Sessions Tab

- Identical to the current UI layout: session panel (left) + pane workspace (right), filtered to the selected project.
- The `selectedProjectId` from the sidebar controls which project's sessions are shown.
- New behaviour: clicking a project in the sidebar now opens that project's sessions across multiple pane slots (see §2.7).

### 5.3 Workspace Tab

- Session list on the left shows **all sessions** (no project filter), grouped by project.
- Session rows can be dragged to pane slots on the right.
- Pane slots can hold sessions from any project.
- Existing `LayoutSwitcher` moves to this tab's header (saves/restores workspace tab layouts).

### 5.4 Overview Tab

- Contains `CrossProjectOverview.svelte` at full width.
- The current "Overview" button in `session-panel-header` is removed (replaced by this tab).
- The `overviewOpen` state in `+page.svelte` is replaced by `activeView === 'overview'`.

### 5.5 Tab Persistence

`active_view: 'sessions' | 'workspace' | 'overview'` stored in `workspaceStore.ui`.

---

## 6. State Persistence and Session Restore

### 6.1 Current Persistence (existing)

`workspaceStore` already persists: `active_project_ids`, `focused_session_id`, `pane_layout`, `ui.scratchpad_visible`.

### 6.2 Extended `ui` Map

The `ui: Record<string, unknown>` field (already in `WorkspaceState`) is extended with:

```typescript
interface WorkspaceUi {
  // Existing
  scratchpad_visible?: boolean;

  // New §5
  active_view?: 'sessions' | 'workspace' | 'overview';

  // New §2 — pane slots in the workspace (and sessions tab)
  workspace_pane_slots?: PaneSlot[];
  sessions_pane_slots?: PaneSlot[];  // per-project or global

  // New §2 — pane widths (paneforge sizes, percent per slot)
  workspace_pane_sizes?: number[];
  sessions_pane_sizes?: number[];

  // New §3
  focus_mode?: 'none' | 'single' | 'split-two';
  focus_mode_session_ids?: number[];

  // New §7
  sidebar_collapsed?: boolean;
}

interface PaneSlot {
  session_id: number;
  split_percent: number;   // agent/companion split within the slot (existing)
  order: number;           // horizontal position
}
```

No backend schema change — all of this is stored inside the existing `payload_json` JSONB of `workspace_state`. The `version` field increments to `2` to distinguish the new structure; old state is migrated forward with sensible defaults on first load.

### 6.3 Ghost Sessions on Workspace Restore

When `workspaceStore.hydrate()` loads a state that includes `pane_slots`, it checks each `session_id` against `sessionsStore`. If a session no longer exists (ended, pruned):

- A **ghost slot** is displayed in place of the real session pane.
- Ghost slot shows: project colour strip, project name, session label, last-known status ("ended"), and the command from `metadata.command[]` in a `<code>` block.
- A **▶ Restart** button calls `sessionSpawn` with the stored project, command, and label. This re-runs the exact same command.
- A **✕ Remove** button removes the ghost slot from the workspace.

This means: if you save a layout with 4 sessions and restart, you see 4 ghost panes each with a Restart button. You can re-launch exactly the sessions you had, one at a time, rather than having 20 sessions start at once.

### 6.4 Named Layouts (existing, extended)

The existing `LayoutSwitcher` saves and restores `WorkspaceState` objects. It already supports multi-layout named saves. Under this spec:

- Layouts now include the full `ui` map (including `pane_slots`).
- Restoring a layout follows the ghost-session logic in §6.3.
- The user can save one layout per "project configuration" and restore it whenever they want that setup back.

---

## 7. Collapsible Sidebar

### 7.1 Behaviour

The project sidebar (260 px) can be collapsed to a 0-width hidden state. A hamburger button (`☰`) in a fixed position at the top-left of the `.main-panel` header is always visible.

Collapsed state:
- Sidebar slides left and becomes invisible (`width: 0; overflow: hidden`).
- The hamburger button stays visible.
- Hover over the hamburger area (48 px wide strip) causes the sidebar to peek out as a fly-out panel over the content (does not compress the content area).
- Clicking the hamburger toggles the permanent-open state.

### 7.2 Implementation

`bits-ui Collapsible` wraps the `Sidebar.svelte`. CSS transition on the content element:
```css
.sidebar-content {
  transition: width 200ms ease, opacity 200ms ease;
}
```

The "peek on hover" behaviour is a separate CSS overlay pattern: the sidebar becomes `position: absolute` (over the content) when it is in the collapsed-but-hovered state, so it does not compress the content area.

### 7.3 Persistence

`sidebar_collapsed: boolean` in `workspaceStore.ui`.

---

## 8. Session Identification in the List

### 8.1 Active-Session Indicator

`SessionRow.svelte` receives a new `active: boolean` prop. When true:

```css
.session-row.active {
  border-left: 2px solid var(--project-color, var(--color-accent));
  background: color-mix(in srgb, var(--project-color, var(--color-accent)) 8%, var(--color-surface));
}

.session-row.active::before {
  content: '';
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--project-color, var(--color-accent));
  flex-shrink: 0;
}
```

When any slot is active, non-active rows reduce label+project text opacity to 70% (`filter: opacity(0.7)` on `.session-main` only — badges remain fully opaque).

### 8.2 Hover-to-Highlight

Hovering a session row, if that session is currently visible in a pane slot, applies a brief highlight to that slot's pane header (CSS class `.pane-header-hover-highlight` for 600 ms via a timeout). This is the browser DevTools "hover element → highlight in viewport" pattern.

### 8.3 Click-to-Flash

Clicking a session row that is already visible in a pane slot causes that pane's border to flash (1.5 s animation from `var(--project-color)` → transparent). Implemented by a `highlighted: boolean` prop on `SplitView` / the pane slot wrapper. This confirms "yes, I clicked the right one" without requiring the user to scan for the active indicator.

The same flash is triggered when AlertBar's "Go to" button activates a session (R4a from research.md §B.4).

### 8.4 Alert → Pane Navigation (AlertBar improvement)

When AlertBar's "Go to" fires:
1. Call `handleActivateSession(session)` (existing).
2. Scroll the session list to make the session row visible (`scrollIntoView({ behavior: 'smooth', block: 'nearest' })`).
3. Flash the pane border (§8.3).
4. Show the alert reason in the pane header badge.

---

## 9. CSS Architecture for Project Colours

Project colours are injected as CSS custom properties on the element that wraps each colour-bearing surface:

```svelte
<!-- In SessionRow.svelte -->
<div
  class="session-row"
  class:active
  style="--project-color: {projectColor}"
>
```

```svelte
<!-- In Sidebar.svelte project item -->
<li
  class="project-item"
  class:selected={selectedProjectId === project.id}
  style="--project-color: {project.settings.color ?? 'var(--color-accent)'}"
>
```

This approach:
- Requires no global colour state management
- Works with CSS `color-mix()` for tinting
- Is trivially serialisable (hex strings already in `settings.color`)
- Does not conflict with the global `--color-accent` variable

---

## 10. Non-Goals for v2

These are explicitly deferred:

- **LLM summarisation of activity**: ring-buffer heuristic stays.
- **Remote sessions** (SSH, cloud, WSL).
- **VS Code / editor companion**.
- **Free-drag floating panels** (moasic-style arbitrary positioning): `@neodrag/svelte` is available for this if it is ever desired; the current design uses paneforge's fixed grid.
- **AI-powered session colour suggestion** based on branch/language.
- **Tab groups within a view** (VS Code tab groups).
- **Two-pane focus mode with different axis splits** (only vertical split for `split-two`).
