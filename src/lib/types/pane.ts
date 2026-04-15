// Phase 4-B/C: shared pane-slot types. Extracted here (rather than living on
// a component's prop shape) so that Phase 4-D (DnD), Phase 4-G (overflow), and
// Phase 5 (ghost sessions) can import the same interface without creating a
// circular import between `PaneWorkspace.svelte` and `+page.svelte`.

/**
 * Phase 5: Snapshot of a session's identity + relaunch arguments, captured
 * while the session was still alive. Persisted alongside the slot so that
 * after a workbench restart — or after the backend prunes the ended row —
 * the pane can render a "ghost" placeholder with a ▶ Restart button that
 * knows which project + command to re-spawn.
 *
 * `project_color` is snapshotted at end time so the ghost still carries its
 * project identity even if the project is later deleted.
 */
export interface GhostSessionData {
	project_id: number;
	label: string;
	/** From `SessionMetadata.command`; may be `[]` for sessions whose metadata
	 *  lacks the command (e.g. wrapper-owned rows). A ghost with an empty
	 *  command renders as non-restartable (button disabled). */
	command: string[];
	/** Snapshot of `getProjectColor(project)` at the time the ghost was
	 *  captured. May be `null` for projects that never had a colour. */
	project_color: string | null;
}

/**
 * One slot in the horizontal pane workspace. Persisted as JSON in
 * `workspace_state.payload_json.ui.workspace_pane_slots` (and, in a later
 * phase, `sessions_pane_slots`).
 *
 * Phase 5 contract change: the session referenced by `session_id` may have
 * been pruned between workbench restarts, **but the slot is no longer
 * silently dropped**. Instead, the slot falls back to rendering as a ghost
 * using `ghost_data`. If `ghost_data` is absent we still render a minimal
 * ghost (label `Session #{id}`, empty command, null colour) rather than
 * dropping — the UI never silently loses layout entries.
 *
 * `ghost_data` is refreshed on every status update while the session is
 * live (see `+page.svelte`'s snapshot effect) so that whenever the session
 * transitions to `ended`/`error` the persisted snapshot is current.
 */
export interface PaneSlot {
	/** Session id currently rendered in the slot. */
	session_id: number;
	/**
	 * Agent/companion split percentage inside the slot (0–100). `SplitView`
	 * currently owns its own divider state, so this field is kept for forward
	 * compatibility but is not read in Phase 4-B/C.
	 */
	split_percent: number;
	/** Horizontal position; smaller = further left. */
	order: number;
	/**
	 * Phase 5: populated while a live session fills this slot (refreshed on
	 * every status update). When the session ends or is no longer in the
	 * store, the slot falls back to ghost mode and uses this snapshot.
	 */
	ghost_data?: GhostSessionData;
}
