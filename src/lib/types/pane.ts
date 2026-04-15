// Phase 4-B/C: shared pane-slot types. Extracted here (rather than living on
// a component's prop shape) so that Phase 4-D (DnD), Phase 4-G (overflow), and
// Phase 5 (ghost sessions) can import the same interface without creating a
// circular import between `PaneWorkspace.svelte` and `+page.svelte`.

/**
 * One slot in the horizontal pane workspace. Persisted as JSON in
 * `workspace_state.payload_json.ui.workspace_pane_slots` (and, in a later
 * phase, `sessions_pane_slots`). The session referenced by `session_id` may
 * have been pruned between workbench restarts — see `PaneWorkspace.svelte`
 * hydration for the filter-missing-sessions behaviour.
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
}
