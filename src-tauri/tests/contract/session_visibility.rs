//! Phase 4-A contract tests: the `session_set_visible` command and the
//! `session_set_focus` shim that wraps it.
//!
//! The Tauri-specific `State<'_, WorkbenchState>` extractor cannot be built
//! in unit tests, so these tests exercise the state mutation helpers
//! (`WorkbenchState::set_visible_sessions`) that both commands now delegate
//! to. The single-shot command glue is trivially covered by compilation.

use std::collections::HashSet;

/// `set_visible_sessions([1, 2])` marks both sessions visible.
#[tokio::test]
async fn set_visible_marks_multiple_sessions() {
    let state = crate::common::mock_state().await;

    state.set_visible_sessions([1, 2]);

    let snap = state.visible_sessions_snapshot();
    assert_eq!(snap, HashSet::from([1, 2]));
}

/// `set_visible_sessions([])` clears the set so no output is forwarded.
#[tokio::test]
async fn set_visible_empty_clears_the_set() {
    let state = crate::common::mock_state().await;

    // Seed with a couple of ids so the clear is observable.
    state.set_visible_sessions([7, 8]);
    assert_eq!(state.visible_sessions_snapshot().len(), 2);

    state.set_visible_sessions(std::iter::empty::<i64>());

    assert!(
        state.visible_sessions_snapshot().is_empty(),
        "empty vec must clear the visible set"
    );
}

/// Subsequent `set_visible_sessions` calls replace (not union) the set.
#[tokio::test]
async fn set_visible_replaces_previous_set() {
    let state = crate::common::mock_state().await;

    state.set_visible_sessions([1, 2, 3]);
    state.set_visible_sessions([4, 5]);

    assert_eq!(state.visible_sessions_snapshot(), HashSet::from([4, 5]));
}

/// The `session_set_focus` shim is equivalent to `set_visible_sessions([id])`
/// when called with `Some(id)`. Exercised via the helper the command now uses.
#[tokio::test]
async fn set_focus_some_acts_like_single_visible() {
    let state = crate::common::mock_state().await;

    // Mirror what `session_set_focus(Some(42))` does internally.
    state.set_visible_sessions(Some(42_i64));

    assert_eq!(state.visible_sessions_snapshot(), HashSet::from([42]));
}

/// `session_set_focus(None)` clears via the shim (equivalent to an empty
/// iterator).
#[tokio::test]
async fn set_focus_none_clears_via_shim() {
    let state = crate::common::mock_state().await;
    state.set_visible_sessions([99_i64]);

    // Mirror what `session_set_focus(None)` does internally.
    state.set_visible_sessions(Option::<i64>::None);

    assert!(state.visible_sessions_snapshot().is_empty());
}
