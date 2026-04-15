// P4-D: AddSlotZone tests. Verifies the drop target dispatches onDrop with
// the session id extracted from the dndzone finalize event.

import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import AddSlotZone from "./AddSlotZone.svelte";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
});

describe("AddSlotZone", () => {
	it("renders a zone with the '+' placeholder when idle", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(AddSlotZone, {
			target,
			props: { onDrop: () => {} },
		});

		const zone = target.querySelector<HTMLElement>(".add-slot-zone");
		expect(zone).not.toBeNull();
		expect(zone!.textContent).toContain("+");
	});

	it("calls onDrop with the session id on finalize", () => {
		const target = document.createElement("div");
		document.body.append(target);
		const drops: number[] = [];
		component = mount(AddSlotZone, {
			target,
			props: { onDrop: (id) => drops.push(id) },
		});
		flushSync();

		const zone = target.querySelector<HTMLElement>(".add-slot-zone")!;

		// Simulate a consider event bringing the dragged item into the zone.
		zone.dispatchEvent(
			new CustomEvent("consider", {
				detail: {
					items: [{ id: "dnd-session-99", sessionId: 99 }],
					info: { trigger: "draggedEntered", id: "dnd-session-99", source: "pointer" },
				},
			}),
		);
		flushSync();

		// Simulate finalize — the user released over this zone.
		zone.dispatchEvent(
			new CustomEvent("finalize", {
				detail: {
					items: [{ id: "dnd-session-99", sessionId: 99 }],
					info: { trigger: "droppedIntoZone", id: "dnd-session-99", source: "pointer" },
				},
			}),
		);
		flushSync();

		expect(drops).toEqual([99]);
	});

	it("ignores the svelte-dnd-action shadow placeholder in finalize", () => {
		const target = document.createElement("div");
		document.body.append(target);
		const drops: number[] = [];
		component = mount(AddSlotZone, {
			target,
			props: { onDrop: (id) => drops.push(id) },
		});
		flushSync();

		const zone = target.querySelector<HTMLElement>(".add-slot-zone")!;

		zone.dispatchEvent(
			new CustomEvent("finalize", {
				detail: {
					items: [{ id: "id:dnd-shadow-placeholder-0000", sessionId: 0 }],
					info: { trigger: "droppedIntoZone", id: "shadow", source: "pointer" },
				},
			}),
		);
		flushSync();

		expect(drops).toEqual([]);
	});

	it("toggles 'hovering' class while a drag is over the zone", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(AddSlotZone, {
			target,
			props: { onDrop: () => {} },
		});
		flushSync();

		const zone = target.querySelector<HTMLElement>(".add-slot-zone")!;
		expect(zone.classList.contains("hovering")).toBe(false);

		zone.dispatchEvent(
			new CustomEvent("consider", {
				detail: {
					items: [{ id: "dnd-session-1", sessionId: 1 }],
					info: { trigger: "draggedEntered", id: "dnd-session-1", source: "pointer" },
				},
			}),
		);
		flushSync();

		expect(zone.classList.contains("hovering")).toBe(true);

		zone.dispatchEvent(
			new CustomEvent("finalize", {
				detail: {
					items: [{ id: "dnd-session-1", sessionId: 1 }],
					info: { trigger: "droppedIntoZone", id: "dnd-session-1", source: "pointer" },
				},
			}),
		);
		flushSync();

		expect(zone.classList.contains("hovering")).toBe(false);
	});
});
