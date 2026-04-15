// P4-E: CommandPalette tests.
//
// Seeds a handful of sessions into the real sessionsStore, mounts the palette
// with `open=true`, and asserts the quick-switch behaviours spelled out in
// `specs/002-adaptive-ui/spec.md §2.6`:
//   - input is auto-focused
//   - all seeded sessions render
//   - typing in the filter narrows the list
//   - ArrowDown moves the roving selection, Enter activates the selected row
//   - Escape calls onClose

import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import CommandPalette from "./CommandPalette.svelte";
import { sessionsStore } from "$lib/stores/sessions.svelte";
import type { SessionSummary } from "$lib/api/sessions";

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
	return {
		id: 0,
		project_id: 1,
		label: "session",
		pid: 1234,
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
		...overrides,
	};
}

let component: ReturnType<typeof mount> | null = null;

/**
 * Seed three sessions spanning two project ids. The palette groups by
 * project_id; since we don't populate the projects store the palette falls
 * back to the `Project {id}` placeholder, which is what `projectNameFor`
 * returns when `projectsStore.byId` misses. That's fine — the filter
 * predicate still matches against the session label.
 */
function seed(): void {
	sessionsStore.add(makeSession({ id: 501, label: "alpha-refactor", project_id: 1 }));
	sessionsStore.add(makeSession({ id: 502, label: "beta-tests", project_id: 1 }));
	sessionsStore.add(makeSession({ id: 503, label: "gamma-docs", project_id: 2 }));
}

function cleanup(): void {
	sessionsStore.remove(501);
	sessionsStore.remove(502);
	sessionsStore.remove(503);
}

describe("CommandPalette", () => {
	beforeEach(() => {
		seed();
	});

	afterEach(() => {
		if (component) {
			unmount(component);
			component = null;
		}
		cleanup();
		document.body.innerHTML = "";
	});

	it("auto-focuses the search input and renders all seeded sessions", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(CommandPalette, {
			target,
			props: { open: true, onClose: () => {}, onActivate: () => {} },
		});
		flushSync();
		// Focus is queued via queueMicrotask; flush microtasks.
		await Promise.resolve();

		const input = target.querySelector<HTMLInputElement>(".palette-input");
		expect(input).not.toBeNull();
		expect(document.activeElement).toBe(input);

		const rows = target.querySelectorAll<HTMLElement>(".palette-row");
		expect(rows.length).toBe(3);
		const labels = Array.from(rows).map((r) =>
			r.querySelector(".palette-label")!.textContent?.trim(),
		);
		expect(labels).toEqual(
			expect.arrayContaining(["alpha-refactor", "beta-tests", "gamma-docs"]),
		);
	});

	it("narrows the list when the user types in the filter", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(CommandPalette, {
			target,
			props: { open: true, onClose: () => {}, onActivate: () => {} },
		});
		flushSync();
		await Promise.resolve();

		const input = target.querySelector<HTMLInputElement>(".palette-input")!;
		input.value = "gamma";
		input.dispatchEvent(new Event("input", { bubbles: true }));
		flushSync();

		const rows = target.querySelectorAll<HTMLElement>(".palette-row");
		expect(rows.length).toBe(1);
		expect(rows[0].querySelector(".palette-label")!.textContent?.trim()).toBe("gamma-docs");
	});

	it("ArrowDown moves the selection and Enter activates the selected session", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		const activated: number[] = [];
		component = mount(CommandPalette, {
			target,
			props: {
				open: true,
				onClose: () => {},
				onActivate: (id) => activated.push(id),
			},
		});
		flushSync();
		await Promise.resolve();

		// On open the first candidate is selected.
		let selected = target.querySelector<HTMLElement>(".palette-row.selected");
		expect(selected).not.toBeNull();
		const firstSelectedId = Number(selected!.dataset.sessionId);

		// ArrowDown moves to the next candidate.
		const overlay = target.querySelector<HTMLElement>(".palette-overlay")!;
		overlay.dispatchEvent(
			new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
		);
		flushSync();

		selected = target.querySelector<HTMLElement>(".palette-row.selected");
		expect(selected).not.toBeNull();
		const secondSelectedId = Number(selected!.dataset.sessionId);
		expect(secondSelectedId).not.toBe(firstSelectedId);

		// Enter activates the currently-selected candidate.
		overlay.dispatchEvent(
			new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
		);
		flushSync();

		expect(activated).toEqual([secondSelectedId]);
	});

	it("Escape calls onClose", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		let closed = 0;
		component = mount(CommandPalette, {
			target,
			props: { open: true, onClose: () => { closed++; }, onActivate: () => {} },
		});
		flushSync();
		await Promise.resolve();

		const overlay = target.querySelector<HTMLElement>(".palette-overlay")!;
		overlay.dispatchEvent(
			new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
		);
		flushSync();

		expect(closed).toBe(1);
	});

	it("clicking a session row calls onActivate with that session id", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		const activated: number[] = [];
		component = mount(CommandPalette, {
			target,
			props: {
				open: true,
				onClose: () => {},
				onActivate: (id) => activated.push(id),
			},
		});
		flushSync();
		await Promise.resolve();

		const row = target.querySelector<HTMLElement>('[data-session-id="502"]');
		expect(row).not.toBeNull();
		row!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
		flushSync();

		expect(activated).toEqual([502]);
	});

	it("does not render when open=false", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(CommandPalette, {
			target,
			props: { open: false, onClose: () => {}, onActivate: () => {} },
		});
		flushSync();

		expect(target.querySelector(".palette-overlay")).toBeNull();
	});
});
