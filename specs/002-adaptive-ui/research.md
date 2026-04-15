# Feature 002 — Adaptive UI: Research Findings

**Feature**: `002-adaptive-ui`
**Date**: 2026-04-15
**Status**: Complete

This document summarises the two research tracks conducted before spec authoring:
(A) Svelte 5 UI library landscape and (B) UX patterns from analogous systems.

---

## A. Svelte 5 UI Library Landscape

### A.1 Drag-and-Drop

| Library | Svelte 5? | Last release | Stars | Notes |
|---|---|---|---|---|
| **svelte-dnd-action** | Yes (compat mode) | ~Jan 2026 | 2,100 | Best pick for _container-based_ list/sortable DnD. ~40k weekly downloads. Patches landed v0.9.44–0.9.69 for Svelte 5. Uses `onconsider`/`onfinalize`. No Tauri issues. |
| **@thisux/sveltednd** | Yes (runes, native) | Apr 2026 | 551 | Zero external deps. Built on Svelte 5 `$state`. HTML5 Drag API + Pointer Events. Very new but actively maintained. |
| **@neodrag/svelte** | Yes (attachments) | ~Aug 2025 | 2,400 | Free-form dragging of individual elements (not lists). Best for floating-panel drag. v2.x uses Svelte 5 `{@attach}` syntax. |
| dnd-kit-svelte | Yes | Feb 2026 | 317 | Port of React's @dnd-kit. Complex API. Overkill for pane reordering. |
| svelte-mosaic | Unknown/No | Abandoned | ~11 | No releases; do not use. |

**Decision**: `svelte-dnd-action` for sortable session-slot ordering. `@neodrag/svelte` is available if free-drag floating panels are ever added.

No viable Svelte mosaic/tile-manager exists. GoldenLayout is framework-agnostic vanilla JS — integrating it with Svelte 5 requires manual lifecycle bridging and is not recommended.

---

### A.2 Resizable Split Panes

| Library | Svelte 5? | Last release | Stars | Notes |
|---|---|---|---|---|
| **paneforge** | Yes (runes) | Aug 2025 | 622 | Part of svecosystem (same org as bits-ui). Svelte 5 rewrite in v1.0.0 (Jun 2025). Supports nested groups, collapsible panes, state persistence. shadcn-svelte's `Resizable` wraps this. Clean API. |
| svelte-splitpanes | Yes (v8.0.9+) | Jan 2026 | 472 | More features (snap-to-size, double-click expand, RTL). Has a reported intermittent `vite-plugin-svelte:optimize-svelte` build error in some Svelte 5 + Vite setups. Worth testing early. |

**Decision**: `paneforge` — svecosystem backing, cleanest Svelte 5 integration, no reported Vite/Tauri build issues. Existing custom divider in `SplitView.svelte` (agent/companion) stays as-is; paneforge is for the outer horizontal layout only.

---

### A.3 Color Picker

| Library | Svelte 5? | Bundle | Notes |
|---|---|---|---|
| **vanilla-colorful** | N/A (Web Component) | 2.7 KB gz | Framework-agnostic Custom Element. 12 color model variants. 100% test coverage, WAI-ARIA compliant, CSS Shadow Parts for styling. Zero deps. Works in Tauri WebView without polyfills. |
| svelte-awesome-color-picker | Yes (v4 rewrite) | ~15 KB | Full-featured: HSV canvas, hex/RGB/HSL inputs, swatches, WCAG contrast. Svelte 5 runes rewrite in v4.0.0. |

**Decision**: `vanilla-colorful` (specifically `<hex-color-picker>` or `<hsl-color-picker>`) for the project colour swatch in the sidebar. 2.7 KB, zero deps, works as a drop-in web component — no Svelte wrapper code needed. Its CSS Shadow Parts allow styling the inner track and thumb to match the dark theme.

---

### A.4 Tabs and Interactive Primitives

| Library | Svelte 5? | Tailwind? | Last release | Stars |
|---|---|---|---|---|
| **bits-ui** | Yes | No | Apr 2026 (v2.17.3) | 3,200 |
| shadcn-svelte | Yes | **Required** | Apr 2026 | — |
| melt-ui | Partial | No | Holding pattern | — |

**Decision**: `bits-ui` directly. It is headless (no Tailwind required), actively maintained (released 7 days before spec date), and provides `Tabs`, `Collapsible`, `Dialog`, `Tooltip`, and every other interactive primitive needed. shadcn-svelte is built on bits-ui but bundles Tailwind — unsuitable for tend's vanilla-CSS architecture. melt-ui is in a holding pattern; the melt maintainers direct new projects to bits-ui.

---

### A.5 Sidebar Collapse

No standalone maintained library exists. Two viable approaches:

1. **`bits-ui Collapsible`** — handles `aria-expanded`, keyboard, and `forceMount` for CSS transitions. Correct primitive for a toggle-with-animation sidebar. Same package as A.4.
2. **`paneforge` with `collapsible` prop** — if the sidebar must also be _drag-resizable_, a paneforge `Pane` with `collapsible={true}` and `collapsedSize={0}` gives both behaviours for free (shadcn-svelte's Resizable Sidebar uses this pattern exactly).

**Decision**: `bits-ui Collapsible` for the animate-to-zero sidebar. If drag-to-resize sidebar width is added later, migrate to a paneforge pane.

---

### A.6 Summary

| Need | Library | Version | Status |
|---|---|---|---|
| Horizontal pane splitting (multi-session layout) | `paneforge` | 1.0.2 | ✅ Adopt |
| Sortable session slot ordering | `svelte-dnd-action` | 0.9.69 | ✅ Adopt |
| Project colour picker | `vanilla-colorful` | latest | ✅ Adopt |
| Tabs, Collapsible, Dialog | `bits-ui` | 2.17.3 | ✅ Adopt |
| Free-drag floating panels | `@neodrag/svelte` | 2.3.3 | ⏳ Future |

---

## B. UX Patterns from Analogous Systems

### B.1 Session Overflow — "More sessions than fit on screen"

**What established tools do:**

- **VS Code**: Tab bar scrolls horizontally; an overflow chevron opens a "show all" quickpick list. The _real_ power-user path is the Command Palette (`Ctrl+P`) — tab visibility becomes secondary. VS Code designers intentionally made the palette the primary navigation path.
- **tmux** `prefix+s` / `choose-session`: An interactive fuzzy-filterable tree of all sessions. The list sidebar is secondary. The "choose tree" picker is the workhorse.
- **iTerm2**: `Cmd+Opt+E` "Open Quickly" palette.
- **Grafana**: Panels in a row can be collapsed to a summary row (count + max severity). Users browse rows, not individual panels when there are many.

**Key insight**: The list panel is for orientation and recency; a _quick-switch palette_ (`Ctrl+K`) is for navigation when the list doesn't fit. Adding session-count badges to project group headers (à la Grafana collapsed row summaries) lets users assess load without scrolling.

**Recommendations:**
- `Ctrl+K` / `Ctrl+Shift+P` quick-switch palette (fuzzy filter, keyboard-only navigation, Enter to activate)
- `/` shortcut to focus the existing filter input
- Session count + alert count badge on project group headers in the session list

---

### B.2 Active vs Inactive Sessions in the List

**What established tools do:**

- **VS Code Explorer**: Open files show a small coloured dot (●). The dot colour matches the editor group's accent. Files not open have no indicator.
- **tmux status bar**: Active window is brighter + `*` suffix. Windows with activity since last visit show `#`.
- **Slack sidebar**: Active channel = filled background row. Unread = bold. Mentions = green badge. Muted = dimmed text.

**Key insight (Nielsen Norman / preattentive attributes research)**: A _left-edge_ colour indicator (dot or border strip) is detected in < 200 ms without active search. Right-edge badges require more directed attention. Reserve animation (pulse) for the highest-urgency state only (`needs_input`).

**Recommendations:**
- Pass `activeSessionIds: Set<number>` from the page to `SessionRow`; active rows get `border-left: 2px solid var(--project-color)` + a small filled dot (●) + subtle background tint.
- When an active session is set, dim text (not badges) of non-active rows to 70% opacity — the Slack "figure-ground separation" pattern.
- Do not animate the active-state indicator; reserve `badge-pulse` for `needs_input` alerts only.

---

### B.3 Identifying Which Pane Belongs to Which Session

**Established patterns:**

- **Browser tab groups** (Chrome, Arc): Persistent colour assignment per group. Same group = same colour every time. Colour appears on both the tab strip and the group label.
- **tmux/i3 pane borders**: Focused pane has a distinct border colour; unfocused panes are dimmer.
- **Browser DevTools**: Hovering an element in the DOM panel highlights the corresponding element in the viewport (hover for continuity).
- **Datadog/Grafana**: Clicking an alert flashes the source panel with a 2-second yellow border animation (`@keyframes highlight: neutral → colored → neutral`).

**Recommendations:**
- Assign a stable colour to each project (not each session). Every session and pane for that project shares the colour — this is the primary list→pane mapping cue.
- Hovering a session row subtly highlights the corresponding pane header (if visible) — DevTools hover-continuity pattern.
- Clicking a session row in the list causes the pane (if already visible) to flash its border for 1.5 s — Grafana "identify which panel changed" pattern.
- Pane headers always show `project / session-label` to prevent "which session am I in?" confusion.

---

### B.4 Toast / Alert → Pane Navigation

**Established patterns:**

- **VS Code diagnostic notifications**: Click = navigate to file + line. Notification contains enough context to find the source. Click = dereference the pointer.
- **Slack/Discord**: Click = navigate directly to the source message. Channel sidebar gains an independent `@mention` badge. Both mechanisms work independently (belt-and-suspenders).
- **PagerDuty**: Alert shows service name prominently. "View in [Service]" = one-click jump. Mental model: notification = pointer, click = dereference.
- **Datadog**: Monitor alert links to the monitor page; preview shows the metric sparkline. "Show me the data behind the alert" reduces false-alarm cognitive load.

**Current state in tend**: `AlertBar`'s "Go to" calls `onActivateSession` correctly but provides no flash/highlight feedback. No scroll-to-show in the session list. OS notification clicks are not wired to session activation.

**Recommendations:**
- "Go to" (AlertBar + OS notification click): activate session, scroll session list to show the row, flash pane border 1.5 s.
- Add alert reason text to the active pane header when `activeSession.alert` is non-null (PagerDuty "why was I navigated here?" pattern).
- OS notification click must emit a `notification:clicked { session_id }` Tauri event that the frontend catches — requires a small Rust-side change in the notification handler.

---

### B.5 Focus Mode

**Established patterns:**

- **VS Code Zen Mode** (`Ctrl+K Z`): Hides activity bar, sidebar, status bar, tabs. Content-only. Exited by `Escape Escape` OR clicking a visible exit button. Zen Mode does NOT hide notifications (cannot miss alerts while focused). `zenMode.restore` re-enters after restart.
- **tmux `prefix+z`** (pane zoom): Temporarily maximises one pane to fill the terminal. Other panes still exist. A status-bar indicator shows `[1 pane hidden]`. The "temporary maximize while preserving state" pattern.
- **Obsidian/Notion distraction-free**: Hides navigation sidebars but keeps a narrow breadcrumb header so the user retains orientation.
- **Replit Zen Mode**: Exit affordance is a semi-transparent `×` always visible in the top-right. Both `Escape` and click work; visible click target is required because terminal focus captures keyboard events.

**NNGroup finding** (2019 Zen Mode study): Users preferred _partial_ Zen Mode (hide sidebar, keep top bar) over full-screen because top-bar notifications remained visible. The benefit is reduced _peripheral motion_, not raw screen area.

**Recommendations:**
- Focus mode hides session panel + sidebar but keeps AlertBar visible.
- Keyboard shortcut: `Ctrl+Shift+F`. Also a ⊙ button in the pane header.
- Always-visible `×` exit button in the top-right corner of the content area (not just `Escape`, because xterm captures keyboard input).
- Show a compact chip (`project / session-label`) at the top of the content area in focus mode — the Obsidian breadcrumb pattern.
- Persist `focus_mode_active` in `workspaceStore.ui` (mirrors `scratchpad_visible` pattern).

---

### B.6 Additional Observations from tend's Code

- **`pane_layout` is currently unused in the frontend** (`WorkspaceState.pane_layout` stores `"split"` but nothing reads it). It is the correct hook to extend for multi-pane / focus mode encoding.
- **AlertBar uses light warning colours** (`#fef3c7`) — a light-mode palette on a dark UI. This is intentional for maximum salience but is visually jarring. VS Code uses a dark-compatible warning colour (`#b89500`). The spec addresses this.
- **`metadata.command[]`** is already stored on sessions (see `SessionMetadata.command`). Ghost-session restart on workspace restore can read this directly — no new backend storage needed.
- **The `missingSessions` Set** in `+page.svelte` is layout-restore-only. Real-time session death is already handled via `session:ended` events; the missing set is a display-only concern for layout restore.
