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

	it("invokes onReorderDragEnd when dragend fires on the drag handle", () => {
		const onReorderDragEnd = vi.fn();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: {
				sessionId: 101,
				onFocus: vi.fn(),
				onClose: vi.fn(),
				// Provide onReorderDragStart so the handle renders as draggable=true
				// (and onReorderDragEnd is therefore wired).
				onReorderDragStart: vi.fn(),
				onReorderDragEnd,
			},
		});

		const handle = target.querySelector<HTMLElement>("[data-drag-handle]");
		expect(handle).not.toBeNull();
		// dragend covers both successful drops and cancelled drags (Escape /
		// drop outside any valid target). PaneWorkspace relies on it to clear
		// reorderDragSessionId so a stale id can't affect the next drag.
		// jsdom doesn't define DragEvent; a plain Event is sufficient because
		// the ondragend handler (onReorderDragEnd) ignores the event object.
		handle!.dispatchEvent(new Event("dragend", { bubbles: true }));
		expect(onReorderDragEnd).toHaveBeenCalledTimes(1);
	});

	it("renders a minimal ghost (does not crash) for a missing session id", () => {
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

		// Phase 5: missing session renders the ghost header instead of live.
		const ghostHeader = target.querySelector<HTMLElement>(
			".pane-slot-header-ghost",
		);
		expect(ghostHeader).not.toBeNull();

		// Ghost label falls back to `Session #<id>` when no ghost_data.
		expect(ghostHeader!.textContent).toContain("Session #999999");

		// Remove (×) button is present.
		const closeBtn = target.querySelector<HTMLButtonElement>(
			".pane-slot-close-btn",
		);
		expect(closeBtn).not.toBeNull();

		// No focus button in ghost mode — there's no live session to focus on.
		const focusBtn = target.querySelector<HTMLButtonElement>(
			".pane-slot-focus-btn",
		);
		expect(focusBtn).toBeNull();

		// Restart button is present but disabled — no command recorded.
		const restartBtn = target.querySelector<HTMLButtonElement>(
			".ghost-restart-btn",
		);
		expect(restartBtn).not.toBeNull();
		expect(restartBtn!.disabled).toBe(true);
	});

	it("enables the Restart button when ghostData has a command", () => {
		const onRestart = vi.fn(async () => 42);
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: {
				sessionId: 999_998,
				onFocus: vi.fn(),
				onClose: vi.fn(),
				ghostData: {
					project_id: 7,
					label: "feature-branch",
					command: ["claude", "--dangerous"],
					project_color: "#60a5fa",
				},
				onRestart,
			},
		});

		// Label + project name come from ghostData.
		const label = target.querySelector<HTMLElement>(".pane-slot-label");
		expect(label!.textContent?.trim()).toBe("feature-branch");

		// Command block shows the stored command.
		const codeEl = target.querySelector<HTMLElement>(".ghost-command code");
		expect(codeEl).not.toBeNull();
		expect(codeEl!.textContent).toBe("claude --dangerous");

		// Restart button is enabled and clickable.
		const restartBtn = target.querySelector<HTMLButtonElement>(
			".ghost-restart-btn",
		);
		expect(restartBtn).not.toBeNull();
		expect(restartBtn!.disabled).toBe(false);
	});

	it("ghost header has a drag handle and fires onReorderDragEnd (N3 symmetry fix)", () => {
		// Phase 5 review N3: ghost slots must be drag sources as well as drop
		// targets. Without this, a layout like [A, ghost(B), C] would allow
		// reordering A or C via drag but not ghost(B), making the workspace
		// inconsistently interactive.
		const onReorderDragStart = vi.fn();
		const onReorderDragEnd = vi.fn();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: {
				sessionId: 999_996, // not in the store → ghost mode
				onFocus: vi.fn(),
				onClose: vi.fn(),
				ghostData: {
					project_id: 7,
					label: "feature-branch",
					command: ["claude"],
					project_color: "#60a5fa",
				},
				onRestart: vi.fn(async () => 42),
				onReorderDragStart,
				onReorderDragEnd,
			},
		});

		// Ghost header should render a drag handle just like the live header.
		const ghostHeader = target.querySelector<HTMLElement>(".pane-slot-header-ghost");
		expect(ghostHeader).not.toBeNull();

		const handle = ghostHeader!.querySelector<HTMLElement>("[data-drag-handle]");
		expect(handle, "ghost header should have a drag handle").not.toBeNull();
		expect(handle!.getAttribute("draggable")).toBe("true");

		// dragend on the ghost handle clears PaneWorkspace reorder state.
		handle!.dispatchEvent(new Event("dragend", { bubbles: true }));
		expect(onReorderDragEnd).toHaveBeenCalledTimes(1);
	});

	it("invokes onRestart when the Restart button is clicked", async () => {
		const onRestart = vi.fn(async () => 101);
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneSlot, {
			target,
			props: {
				sessionId: 999_997,
				onFocus: vi.fn(),
				onClose: vi.fn(),
				ghostData: {
					project_id: 7,
					label: "feature-branch",
					command: ["claude"],
					project_color: null,
				},
				onRestart,
			},
		});

		const restartBtn = target.querySelector<HTMLButtonElement>(
			".ghost-restart-btn",
		);
		restartBtn!.click();
		// The click handler is async; flush microtasks.
		await Promise.resolve();
		await Promise.resolve();
		expect(onRestart).toHaveBeenCalledTimes(1);
	});
});
