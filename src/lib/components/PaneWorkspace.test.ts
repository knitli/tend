// Phase 4-B: PaneWorkspace regression tests.
//
// PaneWorkspace takes a `slots` prop and renders one <Pane> per visible slot.
// We exercise:
//   1. 0 slots → empty-state paragraph (no PaneSlot children)
//   2. 1 slot  → exactly one `.pane-slot` child renders
//   3. 2 slots → two `.pane-slot` children + one resizer between them
//   4. overflow → with a narrow container mock, `data-overflow-count` on the
//      root > 0 so Phase 4-G's popover has something to surface. We mock
//      `ResizeObserver` via the `clientWidth` getter because jsdom's RO
//      implementation doesn't invoke callbacks on mount.
//
// SplitView (inside PaneSlot) will attempt `sessionActivate` on mount and
// fail under jsdom — that's fine, it's caught in SplitView's try/catch and
// doesn't affect header assertions.

import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PaneWorkspace from "./PaneWorkspace.svelte";
import { sessionsStore } from "$lib/stores/sessions.svelte";
import type { SessionSummary } from "$lib/api/sessions";
import type { PaneSlot } from "$lib/types/pane";

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
	// Seed three sessions so the multi-slot test has real rows to point at.
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
		// Force a wide `clientWidth` on every div via the prototype so the
		// ResizeObserver fallback path in PaneWorkspace sees a comfortable
		// container (jsdom reports 0 by default). Restored in afterEach.
		const descriptor = Object.getOwnPropertyDescriptor(
			HTMLElement.prototype,
			"clientWidth",
		);
		Object.defineProperty(HTMLElement.prototype, "clientWidth", {
			configurable: true,
			get: () => 1600,
		});

		try {
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
		} finally {
			if (descriptor) {
				Object.defineProperty(HTMLElement.prototype, "clientWidth", descriptor);
			} else {
				// @ts-expect-error — no original descriptor to restore
				delete HTMLElement.prototype.clientWidth;
			}
		}
	});

	it("renders two PaneSlot children + a resizer for two slots", () => {
		const descriptor = Object.getOwnPropertyDescriptor(
			HTMLElement.prototype,
			"clientWidth",
		);
		Object.defineProperty(HTMLElement.prototype, "clientWidth", {
			configurable: true,
			get: () => 1600,
		});

		try {
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
		} finally {
			if (descriptor) {
				Object.defineProperty(HTMLElement.prototype, "clientWidth", descriptor);
			} else {
				// @ts-expect-error — no original descriptor to restore
				delete HTMLElement.prototype.clientWidth;
			}
		}
	});

	it("reports overflowCount > 0 when the container is too narrow for all slots", () => {
		// Force a narrow container via the prototype `clientWidth` getter so
		// PaneWorkspace's `onMount` read (before any ResizeObserver fires)
		// sees 700 px — below 3 × 520 px minimum so two slots go into
		// overflow. Restored in the finally block.
		const descriptor = Object.getOwnPropertyDescriptor(
			HTMLElement.prototype,
			"clientWidth",
		);
		Object.defineProperty(HTMLElement.prototype, "clientWidth", {
			configurable: true,
			get: () => 700,
		});

		try {
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
			// Container width 700; max visible = floor(700 / 520) = 1.
			expect(root!.getAttribute("data-visible-count")).toBe("1");
			// overflowCount = 3 - 1 = 2.
			expect(Number(root!.getAttribute("data-overflow-count"))).toBeGreaterThan(0);
		} finally {
			if (descriptor) {
				Object.defineProperty(HTMLElement.prototype, "clientWidth", descriptor);
			} else {
				// @ts-expect-error — no original descriptor to restore
				delete HTMLElement.prototype.clientWidth;
			}
		}
	});
});
