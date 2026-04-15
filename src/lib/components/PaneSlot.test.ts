// Phase 4-C: PaneSlot regression tests.
//
// PaneSlot wraps the existing SplitView with a compact header (project dot +
// name, session label, status badge, focus/close buttons, drag handle). The
// header is the Phase 4-specific surface; SplitView itself is covered
// elsewhere (and its `sessionActivate` call fails silently in jsdom, which
// is fine — we're only asserting the header structure here).
//
// Two scenarios are exercised:
//   1. Valid session present in `sessionsStore` → header renders project
//      name + label + status badge; no `.session-not-found` placeholder.
//   2. Missing session id (pruned between restarts) → placeholder renders
//      with only a × close button; component does NOT crash.

import { mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PaneSlot from "./PaneSlot.svelte";
import { sessionsStore } from "$lib/stores/sessions.svelte";
import type { SessionSummary } from "$lib/api/sessions";

let component: ReturnType<typeof mount> | null = null;

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
	return {
		id: 101,
		project_id: 7,
		label: "feature-branch",
		pid: 4321,
		status: "working",
		status_source: "ipc",
		ownership: "workbench",
		started_at: "2026-04-15T12:00:00Z",
		ended_at: null,
		last_activity_at: new Date().toISOString(),
		last_heartbeat_at: null,
		exit_code: null,
		error_reason: null,
		metadata: {},
		working_directory: "/tmp",
		activity_summary: null,
		alert: null,
		reattached_mirror: false,
		...overrides,
	};
}

beforeEach(() => {
	// Seed the sessions store with a known row so the "valid session" case
	// finds it via `sessionsStore.byId`.
	sessionsStore.add(makeSession());
});

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	// Clear seeded state between cases so a previous test doesn't leak.
	sessionsStore.remove(101);
	document.body.innerHTML = "";
});

describe("PaneSlot", () => {
	it("renders the project/session header when the session exists", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: {
				sessionId: 101,
				onFocus: vi.fn(),
				onClose: vi.fn(),
			},
		});

		const root = target.querySelector<HTMLElement>(".pane-slot");
		expect(root).not.toBeNull();

		const header = target.querySelector<HTMLElement>(".pane-slot-header");
		expect(header).not.toBeNull();

		// Session label is rendered in the header.
		const label = target.querySelector<HTMLElement>(".pane-slot-label");
		expect(label).not.toBeNull();
		expect(label!.textContent?.trim()).toBe("feature-branch");

		// Status badge uses the shared status-* class naming so it picks up
		// SessionRow's palette rules at runtime.
		const badge = target.querySelector<HTMLElement>(".badge.status-working");
		expect(badge).not.toBeNull();
		expect(badge!.textContent?.trim()).toBe("Working");

		// Drag handle is rendered (inert in Phase 4-B/C) — Phase 4-D queries
		// for this selector to wire DnD.
		const dragHandle = target.querySelector<HTMLElement>("[data-drag-handle]");
		expect(dragHandle).not.toBeNull();
	});

	it("invokes onClose when the × button is clicked", () => {
		const onClose = vi.fn();
		const onFocus = vi.fn();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: { sessionId: 101, onFocus, onClose },
		});

		const closeBtn = target.querySelector<HTMLButtonElement>(
			".pane-slot-close-btn",
		);
		expect(closeBtn).not.toBeNull();
		closeBtn!.click();
		expect(onClose).toHaveBeenCalledTimes(1);
		expect(onFocus).not.toHaveBeenCalled();
	});

	it("renders a placeholder (and does not crash) for a missing session id", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: {
				sessionId: 999_999, // not in the store
				onFocus: vi.fn(),
				onClose: vi.fn(),
			},
		});

		// The placeholder header text is "Session not found".
		const header = target.querySelector<HTMLElement>(
			".pane-slot-header-missing",
		);
		expect(header).not.toBeNull();
		expect(header!.textContent).toContain("Session not found");

		// Only the × close button should be present — no focus/drag-handle
		// controls because there's no session to focus on.
		const closeBtn = target.querySelector<HTMLButtonElement>(
			".pane-slot-close-btn",
		);
		expect(closeBtn).not.toBeNull();
		const focusBtn = target.querySelector<HTMLButtonElement>(
			".pane-slot-focus-btn",
		);
		expect(focusBtn).toBeNull();
	});
});
