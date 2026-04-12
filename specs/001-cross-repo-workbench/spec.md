# Feature Specification: Cross-Repo Agent Workbench

**Feature Branch**: `001-cross-repo-workbench`
**Created**: 2026-04-11
**Status**: Draft
**Input**: User description: "I find myself needing to manage a large number of windows across multiple projects at the same time. In the age of LLM-driven coding, you spend a lot of time waiting on agents to finish tasks, so you naturally want to multitask. For me, I prefer to multitask across repos. Tools like claude-squad use a tmux tui to allow for multiple sessions but they must all be in the same repo. I have a few core problems -- window management, understanding which window/panel is for which repo/project, finding the one I want, keeping track of tasks/progress across multiple projects, knowing when a session needs my input"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See every active agent session at a glance (Priority: P1)

A developer with agent sessions running across several different repositories opens a single workbench view and immediately sees every live session, which project each one belongs to, what it is currently doing, and whether it is working, idle, or waiting on the human. They can pick the session they want without hunting through terminal tabs or tmux windows.

**Why this priority**: This is the core pain point. Without a unified view that spans repositories, the user cannot answer the most basic question — "what is running right now and where?" — and every other feature loses most of its value. An MVP that delivers only this overview is already more useful than the current workflow of scattered tabs.

**Independent Test**: Start two or more agent sessions in different repositories, open the workbench, and confirm that every session appears in the overview, each is clearly labeled with its project, and each shows a live status (working / idle / needs input). No other feature is required for this to be useful.

**Acceptance Scenarios**:

1. **Given** three agent sessions running in three different repositories, **When** the user opens the workbench, **Then** all three sessions are listed, each showing project name, session label, and current status.
2. **Given** a session was running and has just finished its task, **When** the underlying session goes idle, **Then** its status in the workbench updates to "idle" within a few seconds without the user refreshing.
3. **Given** a new agent session is started in any tracked repository, **When** the workbench is open, **Then** the new session appears automatically without a manual refresh.
4. **Given** a session ends or its process exits, **When** the workbench is open, **Then** the session is removed from the active list (or moved to a recently-ended section) without leaving a stale entry.

---

### User Story 2 - Get alerted when a session needs attention (Priority: P1)

While the user is focused on one repository, an agent in a different repository reaches a point where it needs human input — a question, an approval, a permission prompt, or a failing task. The workbench surfaces that event clearly enough that the user notices without having to poll every window, and can jump straight to the waiting session in one action.

**Why this priority**: The whole point of multitasking across repos is to let the user work on one thing while agents make progress on others. If the user still has to manually check every session to find out who is waiting, the multitasking benefit evaporates. This is tied with User Story 1 as the minimum viable experience.

**Independent Test**: Start an agent session that will block on user input (e.g., a permission prompt or an interactive question), switch focus to an unrelated window, and verify that the workbench produces a noticeable alert for that specific session and offers a one-action way to jump to it.

**Acceptance Scenarios**:

1. **Given** an agent session is running in the background, **When** it transitions into a state that requires user input, **Then** the workbench marks that session as "needs input" and raises a visible alert.
2. **Given** multiple sessions simultaneously need input, **When** the user views the alert area, **Then** each waiting session is listed distinctly with its project and a brief reason if available.
3. **Given** the user acknowledges or responds to a waiting session, **When** that session resumes working, **Then** its alert clears automatically.
4. **Given** the user has configured quiet hours or per-project notification preferences, **When** a session needs input during those hours, **Then** the alert respects the configured preference (e.g., silent but still visible in the workbench).

---

### User Story 3 - Jump to any session with a paired working terminal (Priority: P1)

When the user wants to interact with a specific agent session — for example, "the one in the `marque` repo that is working on the parser" — they can find and focus it in one or two actions, instead of cycling through windows. Selecting a session brings up a split view: the agent session itself alongside a companion terminal already sitting in that session's repository directory, so the user can inspect files, run ad-hoc commands, or answer the agent's questions without leaving the workbench or hunting for the right terminal.

**Why this priority**: The user's real workflow is not just "look at the agent" — it is "look at the agent and immediately poke around its repo to answer questions, check files, and verify things." Without the paired terminal, every interaction still forces the user to manually open a new terminal, `cd` into the right repo, and re-establish context. Fast navigation plus automatic pairing is what makes the workbench usable day-to-day.

**Independent Test**: With at least four sessions across three repositories, select one, and confirm that (a) the matching session is focused in a single action regardless of which tmux window or emulator it originally lives in, and (b) a companion terminal in that session's repo directory is visible in a split view alongside it.

**Acceptance Scenarios**:

1. **Given** several sessions are visible in the workbench, **When** the user types a project name into a filter or search, **Then** only sessions from that project are shown.
2. **Given** the user selects a session entry, **When** they activate it, **Then** the workbench presents a split view showing the underlying agent session and a companion terminal whose working directory is the session's repository root.
3. **Given** a session is activated for the first time and no companion terminal yet exists for it, **When** the workbench brings the session up, **Then** a new companion terminal is automatically created in the correct repo directory and paired with that session.
4. **Given** a session already has a companion terminal from a previous activation, **When** the session is activated again, **Then** the same companion terminal (with its shell state) is reused, not replaced.
5. **Given** a session's underlying window has been closed or hidden by the OS/window manager, **When** the user selects it, **Then** the workbench either restores/reopens it or clearly reports that the session is no longer reachable.
6. **Given** two sessions in the same repository, **When** the user filters by project, **Then** both are shown with labels that let the user distinguish them (e.g., by task, branch, or custom name), and each has its own companion terminal.
7. **Given** the user closes or kills a companion terminal manually, **When** the session is activated again, **Then** the workbench detects the missing terminal and creates a fresh one in the repo directory rather than failing silently.

---

### User Story 4 - Track task and progress state per session (Priority: P2)

For each active agent session, the workbench shows a short, human-readable summary of what that session is currently doing (task title, phase, recent activity, or elapsed time since last output). The user can scan the list and understand progress across every project without opening each session individually.

**Why this priority**: Status ("working / idle / needs input") answers *whether* a session is active. Task context answers *what it is doing*, which is what lets the user prioritize which session to check on next. Valuable and strongly desired, but the tool is still useful without it as long as P1 stories are met. This is **session-scoped, ephemeral agent activity** — it is explicitly distinct from the user-owned Project Scratchpad (User Story 5), which is for the human's own context and survives any individual session.

**Independent Test**: Run sessions doing clearly different work (e.g., one running tests, one editing code, one asking a question) and confirm that the workbench shows a distinct, meaningful summary for each that lets the user tell them apart without opening them.

**Acceptance Scenarios**:

1. **Given** an agent session is actively producing output, **When** the user views the workbench, **Then** a short summary of the latest activity or current task is displayed for that session.
2. **Given** a session has been idle for a while, **When** the user views the workbench, **Then** the elapsed idle time is visible so the user can tell stuck sessions from fresh ones.
3. **Given** a session advertises a task title or goal (via agent-provided metadata), **When** the workbench displays it, **Then** that title is shown alongside status.
4. **Given** a session's output is very long, **When** the summary is displayed, **Then** it is truncated to a readable length without hiding the fact that more detail is available on demand.

---

### User Story 5 - Per-project scratchpad with cross-project overview (Priority: P1)

The user keeps a lightweight, persistent scratchpad for each project — free-form notes to capture "where I am in the overall game" on that project, plus checkable reminders for things to ask about or follow up on later. The workbench isn't trying to mirror the agent's subtasks (the agents track those themselves); it's giving the *human* a place to stash their own context and open loops. A cross-project overview rolls up every open reminder across every registered project so the user can answer "what am I in the middle of, everywhere?" without opening a single session.

**Why this priority**: The user explicitly listed "keeping track of tasks/progress across multiple projects" and "anything I want to remember to ask about/do for later" alongside window management as a core pain point. Without this, the workbench answers "what are my agents doing?" but not "what am *I* doing / what do I owe myself?" — which is the question the user actually has when they come back to the workbench after hours away from it.

**Independent Test**: Add a few notes and open reminders to two different projects, close the workbench (or reboot), reopen it, open the cross-project overview, and confirm that every note and open reminder is still present and grouped by project.

**Acceptance Scenarios**:

1. **Given** a registered project, **When** the user opens that project's view in the workbench, **Then** a persistent scratchpad is visible (empty or populated) and accepts new entries.
2. **Given** an open scratchpad, **When** the user types a free-form note, **Then** the note is saved with a timestamp and survives workbench restart.
3. **Given** an open scratchpad, **When** the user adds a reminder item, **Then** it appears as a checkable entry in the "open" state and is persisted.
4. **Given** open reminders exist in multiple projects, **When** the user opens the cross-project overview, **Then** every open reminder is listed grouped by project, with its age visible.
5. **Given** a reminder is marked done, **When** the user opens the cross-project overview, **Then** the done reminder no longer appears in the active list but is still retrievable from the originating project's scratchpad history.
6. **Given** the workbench is closed and reopened (or the system is rebooted), **When** the user opens any project, **Then** that project's scratchpad is intact with all notes and reminders in their prior state.
7. **Given** a session ends or the companion terminal is killed, **When** the user opens the project, **Then** the scratchpad for that project is unchanged — scratchpad lifetime is tied to the project, not to any individual session.
8. **Given** the user activates a session, **When** the split view comes up, **Then** the scratchpad for that session's project is reachable from the activation view without leaving it (e.g., as a toggleable panel alongside the agent session and companion terminal).
9. **Given** a reminder has been open for a long time without being checked off, **When** the user views it in either the project scratchpad or the cross-project overview, **Then** its age is surfaced so the user can spot stale items at a glance.

---

### User Story 6 - Persistent workspace state across restarts (Priority: P1)

The user closes the workbench at the end of the day (or reboots, or kills the process) and the next time they open it, their set of registered projects and their last active workspace are there waiting — no manual re-import, no "add project" step, no re-attaching to each terminal one by one. They can also save one or more *named* layouts (e.g., "shipping marque", "client-X investigations") and switch between them explicitly without re-defining projects each time.

**Why this priority**: The user specifically called out that they should be able to pull up a workspace with a set of projects without redefining it every start. Daily friction here negates the entire value proposition — if every morning begins by rebuilding the workbench, people stop using it. Automatic state persistence is part of the minimum experience; named layouts are a convenience on top.

**Independent Test**: Open the workbench, register several projects, start a companion terminal or two, close the workbench (or reboot), reopen it, and confirm that the same projects are loaded and the previous workspace state is restored without manual re-entry. Optionally save a named layout, switch to a different layout, and switch back.

**Acceptance Scenarios**:

1. **Given** the user has registered three projects and has several sessions visible, **When** they close and relaunch the workbench, **Then** the same projects are registered and the workbench restores the last workspace state without any manual re-import.
2. **Given** the workbench is reopened after a reboot, **When** still-running sessions and companion terminals exist for those projects, **Then** the workbench reattaches to them and displays their live status; sessions that did not survive the reboot are clearly marked as "not running".
3. **Given** the user has a current set of sessions and repositories open, **When** they save a named layout, **Then** the layout is persisted locally and appears in a list of saved layouts.
4. **Given** a saved layout exists, **When** the user restores it, **Then** the workbench presents the same repositories and session slots, reconnecting to any sessions that are still alive.
5. **Given** a layout references a session that no longer exists, **When** the layout is restored, **Then** the missing session is clearly marked as "not running" rather than silently dropped.
6. **Given** the user has never saved a named layout, **When** they launch the workbench, **Then** it still restores the most recent workspace state automatically (i.e., automatic persistence does not require explicit layout save).

---

### Edge Cases

- A repository is renamed, moved, or deleted on disk while the workbench is tracking sessions inside it.
- Two repositories on the same machine share the same folder name but live in different parent directories — the workbench must not confuse them.
- A session is started outside the workbench (e.g., directly in a terminal) and the user wants it to show up without extra setup.
- An agent session crashes or its host process is killed; the workbench must detect and reflect the dead state instead of showing it as "working".
- The user closes the companion terminal manually — the workbench must transparently recreate it on next activation instead of failing.
- The user `cd`s out of the repo inside the companion terminal; the workbench must not "correct" this (it's the user's terminal) but must also not lose track of which session the terminal is paired with.
- A session targets a git worktree or submodule; the companion terminal must open in the exact working directory of the agent, not the root of the parent repo.
- The user has many simultaneous sessions (up to the supported ceiling); the overview must remain scannable and not degrade into noise.
- The user's notification channel (OS notifications, terminal bell, in-app badge) is unavailable or muted; "needs input" state must still be visible somewhere non-intrusive.
- A session produces a huge amount of output very quickly; status summaries must not lag or block the workbench.
- Two sessions in the workbench both report "needs input" at the same instant; the user must be able to distinguish and address each.
- The workbench itself is closed and reopened; it must reattach to still-running sessions and their companion terminals rather than leaving them orphaned.
- The machine is rebooted; sessions and their companion terminals will be gone, and the workbench must restore the registered projects and clearly mark the lost sessions as "not running" without requiring the user to re-import anything.
- A project is removed from the workbench; its scratchpad must be archived (not silently deleted), so re-adding the project can restore its notes and reminders.
- A project's scratchpad grows very long over months of use; the workbench must remain responsive when opening or searching it.
- A reminder sits open for weeks or months without being checked off; the workbench must make its age obvious so the user can decide whether it is still meaningful.
- The user adds a reminder while the project has no active session; this must be allowed — scratchpad edits do not require a live session.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The workbench MUST present a unified view of all agent sessions currently active across every repository the user has registered or opened with it.
- **FR-002**: Each session entry MUST be clearly and unambiguously labeled with the project/repository it belongs to.
- **FR-003**: Each session entry MUST display a live status indicating at minimum: `working`, `idle`, and `needs_input`. Additional states (`ended`, `error`) MAY be shown. State detection is two-tier: Tier 1 is cooperative IPC from the agent or the `agentui run` wrapper (authoritative when present); Tier 2 is a best-effort output-activity and prompt-pattern heuristic for non-cooperating sessions. The Tier 2 pattern set is defined in `research.md §7`, its false-positive ceiling is enforced by SC-011, and Tier 2 MUST be suppressed for any session that has ever emitted a cooperative Tier 1 status event in its lifetime.
- **FR-004**: The workbench MUST update session state (appearance, status changes, disappearance) without requiring the user to manually refresh.
- **FR-005**: The workbench MUST raise a noticeable alert when any session transitions into a "needs input" state, and the alert MUST identify which session and which project is waiting.
- **FR-006**: Users MUST be able to filter or search the session list by project/repository name and by session label.
- **FR-007**: Users MUST be able to select a session entry and, with a single action, activate it such that its underlying agent session and its paired companion terminal are both visible together in a split view.
- **FR-008**: The workbench MUST handle sessions that are started outside of it (e.g., manually launched via the `agentui run` CLI wrapper) so they appear in the unified view without requiring the user to re-launch them from the workbench. Such **wrapper-owned** sessions are visible and observable from the workbench (output mirroring, status, alerts, companion terminal pairing, activity summary) but are **read-only** from the workbench — input (keystrokes, resize, signal delivery) MUST be sent through the launching terminal that the wrapper is proxying. Sessions the user spawns directly from the workbench UI (`session_spawn`) are **workbench-owned** and accept input from the workbench normally.
- **FR-009**: The workbench MUST detect when a session has ended or crashed and reflect that state, rather than showing the session as "working" indefinitely.
- **FR-010**: Users MUST be able to distinguish between multiple sessions that live in the same repository (e.g., different task names, branches, or custom labels).
- **FR-011**: The workbench MUST provide a short, human-readable activity or task summary per session so the user can understand what each session is doing without opening it.
- **FR-012**: The workbench MUST allow the user to acknowledge or dismiss "needs input" alerts on a per-session basis, and MUST clear the alert automatically once the session resumes working.
- **FR-013**: The workbench MUST allow the user to configure per-project or global notification preferences (e.g., alert channel, quiet hours) for "needs input" events.
- **FR-014**: The workbench MUST preserve the identity and project mapping of a session across its lifetime — including across workbench restart, crash-recovery reattach, and layout restore — so that a given session always resolves back to the same project, label, working directory, and (where applicable) the same `ownership` classification even after its live in-memory handle has been re-created. "Identity" here means the persistent row id plus the tuple `(project_id, label, working_directory, ownership)`; it does NOT mean the OS-level PTY master fd, which cannot be resurrected across a workbench process restart (see FR-020).
- **FR-015**: The workbench MUST automatically pair each tracked session with a companion terminal whose initial working directory is that session's repository root (or the session's actual working directory if it differs, e.g., worktrees and submodules).
- **FR-016**: The workbench MUST reuse an existing companion terminal for a session on subsequent activations, preserving shell state, and MUST transparently recreate the companion terminal if it has been closed or killed.
- **FR-017**: The workbench MUST NOT force the companion terminal's working directory to remain at the repo root — the user is free to `cd` anywhere inside it — while still keeping the terminal's pairing to its session intact.
- **FR-018**: Users MUST be able to save and later restore named multi-project workspace layouts describing which repositories and session slots are part of each layout.
- **FR-019**: The workbench MUST automatically persist the current set of registered projects and workspace state, and MUST restore them on the next launch without requiring the user to explicitly save a layout or re-import projects.
- **FR-020**: On relaunch, the workbench MUST reconnect to any still-running sessions and their companion terminals, and MUST clearly mark as "not running" any sessions that did not survive the restart. Reattachment has a **read-only caveat for v1**: reattached **workbench-owned** sessions MUST be presented as read-only mirrors (`reattached_mirror = true`, input rejected with `SESSION_READ_ONLY`) because the workbench cannot resurrect the original PTY master fd across a process restart — the user regains input only by ending such a session and spawning a fresh one from the workbench. Reattached **wrapper-owned** sessions retain their existing read-only semantics (input continues to go through the launching terminal, unchanged). Both cases count as "still running" for the purposes of the "not running" marking.
- **FR-021**: The workbench MUST scale to at least 10 concurrent sessions across at least 5 repositories without the overview becoming unscannable or lagging meaningfully.
- **FR-022**: The workbench MUST support sessions and companion terminals that live on the local machine only. Tracking sessions on remote machines — including remote SSH hosts and WSL-to-Windows cross-boundary cases — is explicitly out of scope for v1.
- **FR-023**: The workbench MUST store user configuration (registered projects, layouts, notification preferences, last workspace state, and project scratchpads) locally and reload it on next launch.
- **FR-024**: Each registered project MUST have a persistent Project Scratchpad in which the user can capture free-form notes and checkable reminders.
- **FR-025**: The scratchpad MUST support at minimum two kinds of entries: free-form Notes (timestamped, editable) and Reminders (checkable items with an "open" / "done" state).
- **FR-026**: Scratchpad lifetime MUST be tied to the project, not to any individual session — ending, killing, or restarting a session MUST NOT affect the project's scratchpad contents.
- **FR-027**: The workbench MUST NOT automatically populate the scratchpad from agent output; scratchpads are for the human's own context. Agent-reported activity belongs to the session activity summary (FR-011), not the scratchpad.
- **FR-028**: The workbench MUST provide a cross-project overview that aggregates every open Reminder across every registered project, grouped by project, so the user can see at a glance what they owe themselves everywhere.
- **FR-029**: Users MUST be able to mark a Reminder as done; done Reminders MUST be removed from the active cross-project overview but MUST remain retrievable from the originating project's scratchpad history.
- **FR-030**: The scratchpad for a given project MUST be reachable without leaving the session activation view (e.g., as a toggleable panel alongside the agent session and companion terminal in the split view) and MUST also be reachable from a standalone per-project view independent of any active session.
- **FR-031**: The workbench MUST surface a visible age indicator for open Reminders so the user can identify stale items in both per-project and cross-project views.
- **FR-032**: When a project is removed from the workbench, its scratchpad MUST be archived rather than deleted, so that re-adding the project restores its notes and reminders.

### Key Entities *(include if feature involves data)*

- **Project**: A tracked repository or working directory. Has a stable identity (path + display name), may contain zero or more active sessions, and may have per-project settings such as notification preferences.
- **Session**: A running agent instance tied to exactly one project. Has a label, a status (working / idle / needs-input / ended / error), an **ownership** (workbench-owned or wrapper-owned — see FR-008), a current activity summary, timestamps for last activity and last user attention, a reference to its underlying window/pane so the workbench can focus it, and a reference to a paired Companion Terminal. Wrapper-owned sessions are observable but read-only from the workbench; workbench-owned sessions accept input from the workbench.
- **Companion Terminal**: A user-facing shell session whose initial working directory is the Session's repository root (or worktree/submodule path where applicable). One-to-one with a Session. Persists across activations so shell state is not lost; the workbench recreates it if it has been closed or killed.
- **Workspace State**: The current set of registered Projects, active Sessions, and their companion terminals — the thing the workbench auto-saves on exit and auto-restores on launch, independent of any named layout.
- **Alert**: A transient event raised by a session — at minimum a "needs input" signal — with a reference to the session, a timestamp, an optional reason, and an acknowledged flag.
- **Layout**: A saved, named configuration describing which projects and which session slots belong to a workspace the user wants to restore later. Distinct from Workspace State, which is the automatic "last session" restore point.
- **Notification Preference**: Per-project or global settings that determine how alerts are delivered to the user (channel, quiet hours, severity filter).
- **Project Scratchpad**: A per-project, persistent, user-owned container for human context about that project. Holds Notes and Reminders. Lives as long as the project is registered (and is archived rather than deleted when a project is removed). Not derived from agent output.
- **Note**: A free-form, timestamped text entry inside a Project Scratchpad. For capturing context, thoughts, and "where I am in the overall game" for that project.
- **Reminder**: A checkable entry inside a Project Scratchpad with an "open" or "done" state, a creation timestamp (used to surface age), and optional text. Represents something the user wants to remember to ask about or follow up on.
- **Cross-Project Overview**: A roll-up view aggregating every open Reminder across every registered project, grouped by project, so the user can see everything they owe themselves in one place.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: From opening the workbench, a user with sessions running in at least three different repositories can identify every active session and which project it belongs to within 5 seconds, without opening any individual session.
- **SC-002**: When any agent session enters a "needs input" state, the user becomes aware of it within 10 seconds without manually checking that session.
- **SC-003**: A user can activate a specific target session (by project or label) in no more than two actions, and upon activation the paired companion terminal in that session's repo directory is visible in the split view without any additional user action.
- **SC-004**: The workbench remains responsive — list updates, filters, and activation actions complete without perceptible lag — while tracking at least 10 concurrent sessions across at least 5 repositories.
- **SC-005**: When the workbench is relaunched after being closed (without reboot), the user's registered projects and last workspace state are restored automatically and require zero manual re-import steps.
- **SC-006**: Over a representative working session, the user does not miss a "needs input" event (i.e., no session sits in "needs input" for longer than the user's configured quiet-hours policy allows before the user notices it).
- **SC-007**: When the workbench is closed and reopened without a reboot, at least 95% of still-running sessions and their companion terminals are reattached and displayed correctly without the user having to intervene.
- **SC-008**: A user with scratchpads across multiple projects can answer "what am I in the middle of, everywhere?" by opening the cross-project overview in under 15 seconds, without opening any individual session.
- **SC-009**: Adding a note or reminder to the current project takes no more than two actions from anywhere in the workbench.
- **SC-010**: Open reminders persist indefinitely across workbench restarts and system reboots until the user marks them done; zero open reminders are lost due to lifecycle events (session ending, reboot, workbench restart).
- **SC-011**: For sessions that do not emit cooperative IPC status events, the fallback `needs_input` heuristic MUST produce no more than **1 spurious `needs_input` alert per 100 sessions** across the committed adversarial PTY-output corpus under `tests/fixtures/ptyoutput/`. This is a hard gate: cooperative-IPC sessions are exempt (Tier 1 is load-bearing only for non-cooperating agents).

## Assumptions

- The user runs multiple agent sessions on a single primary workstation (local machine). Remote-session tracking — including remote SSH hosts and WSL-to-Windows cross-boundary cases — is explicitly out of scope for v1, though it is acknowledged as a desirable future enhancement.
- Each tracked agent session exposes, or can be wrapped to expose, enough signal for the workbench to infer at least three states (working, idle, needs-input) — e.g., via output activity, process state, or an integration hook.
- The workbench does not need to host or replace the agent itself; it observes and navigates existing agent sessions.
- The companion terminal is a plain shell process launched by the workbench; it is terminal-only for v1. Richer pairings (e.g., full VS Code / editor integration per session) are explicitly out of scope for v1 and may be revisited later.
- The workbench can programmatically spawn terminal processes with a chosen initial working directory and can present them in a split view alongside an agent session's window/pane.
- The user has a window manager, terminal multiplexer, or OS-level window API capable of bringing a specific terminal/window/pane to the foreground when requested.
- Notification delivery uses whichever channels are standard on the user's platform (OS notifications, in-app alerts, terminal bell) and does not require a cloud service.
- Configuration, saved layouts, and automatically persisted workspace state are stored on the local machine; no server-side sync is required for v1.
- "Project" is equivalent to "repository/working directory" for the purposes of this feature; multi-repo monorepos are treated as a single project unless the user registers subdirectories explicitly.
- A reasonable upper bound of ~10 concurrent sessions across ~5 repositories is assumed for v1 scale targets; beyond that, graceful degradation is acceptable but not required.
- **Assumed v1 goal (validate in beta, not a shipping gate)**: daily setup time to get back to a multi-project working state drops by at least 50 % compared to manually reopening terminals and sessions. Not reproducible without a baseline study, so it is tracked here rather than as a Success Criterion.
- **Assumed v1 goal (validate in beta, not a shipping gate)**: a first-time user can register a project and see their first agent session appear — with companion terminal paired — in under 3 minutes without reading documentation. Requires a usability study to validate; tracked here rather than as a Success Criterion.
- The Project Scratchpad is for the *human's* own context-keeping. The workbench does not attempt to track, mirror, or deduplicate the agent's internal subtasks; agents manage those themselves and the workbench stays out of the way.
- Scratchpad content is lightweight text (optionally with light markdown-style formatting). Rich media — images, file attachments, embedded diagrams — is explicitly out of scope for v1.
- A Reminder's "age" is measured from its creation timestamp; the workbench does not try to infer urgency or priority automatically. The user decides what is stale by looking at the age indicator.
