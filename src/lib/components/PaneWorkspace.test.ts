// Phase 4-B: PaneWorkspace regression tests.
//
// PaneWorkspace takes a `slots` prop and renders one PaneSlot per visible slot
// inside a CSS grid. We exercise:
//   1. 0 slots → empty-state paragraph (no PaneSlot children)
//   2. 1 slot  → exactly one `.pane-slot` child renders
//   3. 2 slots → two `.pane-slot` children in grid cells
//   4. overflow → with a short container mock, `data-overflow-count` on the
//      root > 0. The grid layout uses container HEIGHT (not width) to decide
//      overflow — we mock `clientHeight` via the prototype getter because
//      jsdom's ResizeObserver doesn't invoke callbacks on mount.
//
// SplitView (inside PaneSlot) will attempt `sessionActivate` on mount and
// fail under jsdom — that's fine, it's caught in SplitView's try/catch and
// doesn't affect header assertions.

import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SessionSummary } from "$lib/api/sessions";
import { sessionsStore } from "$lib/stores/sessions.svelte";
import type { PaneSlot } from "$lib/types/pane";
import PaneWorkspace from "./PaneWorkspace.svelte";

let component: ReturnType<typeof mount> | null = null;

function makeSession(id: number): SessionSummary {
	return {
		id,
		project_id: 1,
		label: `session-${id}`,
		pid: 1000 + id,
		status: "idle",
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
	};
}

function makeSlot(sessionId: number, order: number): PaneSlot {
	return { session_id: sessionId, split_percent: 65, order };
}

beforeEach(() => {
	sessionsStore.add(makeSession(1));
	sessionsStore.add(makeSession(2));
	sessionsStore.add(makeSession(3));
});

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	sessionsStore.remove(1);
	sessionsStore.remove(2);
	sessionsStore.remove(3);
	document.body.innerHTML = "";
});

/** Override `clientHeight` on HTMLElement.prototype for the duration of `run`.
 *  The grid layout's overflow is height-based (not width-based). */
function withContainerHeight(height: number, run: () => void): void {
	const descriptor = Object.getOwnPropertyDescriptor(
		HTMLElement.prototype,
		"clientHeight",
	);
	Object.defineProperty(HTMLElement.prototype, "clientHeight", {
		configurable: true,
		get: () => height,
	});
	try {
		run();
	} finally {
		if (descriptor) {
			Object.defineProperty(HTMLElement.prototype, "clientHeight", descriptor);
		} else {
			// @ts-expect-error — no original descriptor to restore
			delete HTMLElement.prototype.clientHeight;
		}
	}
}

describe("PaneWorkspace", () => {
	it("renders the empty-state when slots is empty", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(PaneWorkspace, {
			target,
			props: {
				slots: [],
				onSlotClose: vi.fn(),
				onSlotFocus: vi.fn(),
			},
		});

		const empty = target.querySelector<HTMLElement>(".pane-workspace-empty");
		expect(empty).not.toBeNull();
		expect(target.querySelectorAll(".pane-slot").length).toBe(0);
	});

	it("renders exactly one PaneSlot when slots has length 1", () => {
		withContainerHeight(900, () => {
			const target = document.createElement("div");
			document.body.append(target);

			component = mount(PaneWorkspace, {
				target,
				props: {
					slots: [makeSlot(1, 0)],
					onSlotClose: vi.fn(),
					onSlotFocus: vi.fn(),
				},
			});

			flushSync();
			const panes = target.querySelectorAll<HTMLElement>(".pane-slot");
			expect(panes.length).toBe(1);
		});
	});

	it("renders two PaneSlot children in grid cells for two slots", () => {
		withContainerHeight(900, () => {
			const target = document.createElement("div");
			document.body.append(target);

			component = mount(PaneWorkspace, {
				target,
				props: {
					slots: [makeSlot(1, 0), makeSlot(2, 1)],
					onSlotClose: vi.fn(),
					onSlotFocus: vi.fn(),
				},
			});

			flushSync();
			const panes = target.querySelectorAll<HTMLElement>(".pane-slot");
			expect(panes.length).toBe(2);

			const root = target.querySelector<HTMLElement>(".pane-workspace");
			expect(root?.getAttribute("data-visible-count")).toBe("2");
			expect(root?.getAttribute("data-overflow-count")).toBe("0");
		});
	});

	// Overflow tests: force a short container so not all rows fit at
	// MIN_PANE_HEIGHT_PX (180px). With 3 slots (< 4, so 1 column),
	// maxRows = floor(300/180) = 1, so max visible = 1, overflow = 2.
	it("renders the [+N more] overflow trigger when overflowCount > 0", () => {
		withContainerHeight(300, () => {
			const target = document.createElement("div");
			document.body.append(target);

			component = mount(PaneWorkspace, {
				target,
				props: {
					slots: [makeSlot(1, 0), makeSlot(2, 1), makeSlot(3, 2)],
					onSlotClose: vi.fn(),
					onSlotFocus: vi.fn(),
					onReorderSlots: vi.fn(),
				},
			});

			flushSync();
			const trigger = target.querySelector<HTMLButtonElement>(
				".pane-overflow-trigger",
			);
			expect(trigger).not.toBeNull();
			// 3 slots, 1 column, floor(300/180)=1 row → 1 visible, 2 overflow
			expect(trigger!.textContent?.trim()).toBe("+2 more");
			expect(trigger!.getAttribute("aria-expanded")).toBe("false");
		});
	});

	it("opens the overflow popover on trigger click and lists hidden slots", () => {
		withContainerHeight(300, () => {
			const target = document.createElement("div");
			document.body.append(target);

			component = mount(PaneWorkspace, {
				target,
				props: {
					slots: [makeSlot(1, 0), makeSlot(2, 1), makeSlot(3, 2)],
					onSlotClose: vi.fn(),
					onSlotFocus: vi.fn(),
					onReorderSlots: vi.fn(),
				},
			});

			flushSync();
			const trigger = target.querySelector<HTMLButtonElement>(
				".pane-overflow-trigger",
			);
			trigger!.click();
			flushSync();

			const popover = target.querySelector<HTMLElement>(
				".pane-overflow-popover",
			);
			expect(popover).not.toBeNull();
			expect(trigger!.getAttribute("aria-expanded")).toBe("true");
			const items = popover!.querySelectorAll<HTMLButtonElement>(
				".pane-overflow-item",
			);
			// Slots 2 and 3 are hidden (max visible = 1).
			expect(items.length).toBe(2);
			expect(items[0]!.textContent).toContain("session-2");
			expect(items[1]!.textContent).toContain("session-3");
		});
	});

	it("swaps a hidden slot with the rightmost visible slot via onReorderSlots", () => {
		withContainerHeight(300, () => {
			const target = document.createElement("div");
			document.body.append(target);
			const onReorderSlots = vi.fn();

			component = mount(PaneWorkspace, {
				target,
				props: {
					slots: [makeSlot(1, 0), makeSlot(2, 1), makeSlot(3, 2)],
					onSlotClose: vi.fn(),
					onSlotFocus: vi.fn(),
					onReorderSlots,
				},
			});

			flushSync();
			// Open popover.
			target
				.querySelector<HTMLButtonElement>(".pane-overflow-trigger")!
				.click();
			flushSync();

			// Click the second hidden entry (absolute index 2 — session 3).
			const items = target.querySelectorAll<HTMLButtonElement>(
				".pane-overflow-item",
			);
			items[1]!.click();
			flushSync();

			expect(onReorderSlots).toHaveBeenCalledTimes(1);
			const next = onReorderSlots.mock.calls[0]![0] as PaneSlot[];
			expect(next.map((s) => s.session_id)).toEqual([3, 2, 1]);
			expect(next.map((s) => s.order)).toEqual([0, 1, 2]);

			// Popover should be closed after the swap.
			expect(target.querySelector(".pane-overflow-popover")).toBeNull();
		});
	});

	it("reports overflowCount > 0 when the container is too short for all slots", () => {
		withContainerHeight(300, () => {
			const target = document.createElement("div");
			document.body.append(target);

			component = mount(PaneWorkspace, {
				target,
				props: {
					slots: [makeSlot(1, 0), makeSlot(2, 1), makeSlot(3, 2)],
					onSlotClose: vi.fn(),
					onSlotFocus: vi.fn(),
				},
			});

			flushSync();
			const root = target.querySelector<HTMLElement>(".pane-workspace");
			expect(root).not.toBeNull();
			expect(root!.getAttribute("data-slot-count")).toBe("3");
			// Container height 300; 1 col; max rows = floor(300/180) = 1.
			expect(root!.getAttribute("data-visible-count")).toBe("1");
			expect(Number(root!.getAttribute("data-overflow-count"))).toBeGreaterThan(
				0,
			);
		});
	});
});
