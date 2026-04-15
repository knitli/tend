// T100 + P1-B: Vitest tests for SessionList.
//
// Part 1 — the unified filter predicate (existing T100 coverage). M7 moved the
// predicate into `$lib/util/filterSession.ts`; these tests pin the behaviour.
//
// Part 2 — P1-B integration: mounts the real SessionList, seeds a session into
// the store, dispatches `tend:session-scroll-to` on the window, and verifies
// that `Element.prototype.scrollIntoView` is called with the expected options.

import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SessionList from "./SessionList.svelte";
import { matchesSessionFilter } from "$lib/util/filterSession";
import { sessionsStore } from "$lib/stores/sessions.svelte";
import type { SessionSummary } from "$lib/api/sessions";

describe("SessionList filter predicate", () => {
	// Seed data:
	// Project "alpha project" with session "beta refactor"
	// Project "beta project" with session "alpha rewrite"
	// Project "gamma project" with session "gamma init"

	const sessions = [
		{ label: "beta refactor", projectName: "alpha project" },
		{ label: "alpha rewrite", projectName: "beta project" },
		{ label: "gamma init", projectName: "gamma project" },
	];

	it("empty query matches everything", () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("", s.label, s.projectName),
		);
		expect(results).toHaveLength(3);
	});

	it('"alpha" matches both sessions with alpha in label or project name', () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("alpha", s.label, s.projectName),
		);
		expect(results).toHaveLength(2);
		// "beta refactor" matched via project name "alpha project"
		expect(results[0].label).toBe("beta refactor");
		// "alpha rewrite" matched via session label
		expect(results[1].label).toBe("alpha rewrite");
	});

	it('"beta" matches both sessions with beta in label or project name', () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("beta", s.label, s.projectName),
		);
		expect(results).toHaveLength(2);
		expect(results[0].label).toBe("beta refactor");
		expect(results[1].label).toBe("alpha rewrite");
	});

	it('"gamma" matches only the gamma session', () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("gamma", s.label, s.projectName),
		);
		expect(results).toHaveLength(1);
		expect(results[0].label).toBe("gamma init");
	});

	it('"nonexistent" matches nothing', () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("nonexistent", s.label, s.projectName),
		);
		expect(results).toHaveLength(0);
	});

	it("filter is case-insensitive", () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("ALPHA", s.label, s.projectName),
		);
		expect(results).toHaveLength(2);
	});

	it("whitespace-only query matches everything", () => {
		const results = sessions.filter((s) =>
			matchesSessionFilter("   ", s.label, s.projectName),
		);
		expect(results).toHaveLength(3);
	});
});

// ---------------------------------------------------------------------------
// P1-B integration: tend:session-scroll-to event wiring.
// ---------------------------------------------------------------------------

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
	return {
		id: 123,
		project_id: 1,
		label: "scroll-target",
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

describe("SessionList scroll-to event wiring", () => {
	beforeEach(() => {
		sessionsStore.add(makeSession({ id: 123 }));
	});

	afterEach(() => {
		if (component) {
			unmount(component);
			component = null;
		}
		sessionsStore.remove(123);
		document.body.innerHTML = "";
		vi.restoreAllMocks();
	});

	it("calls scrollIntoView on the matching row when the event fires", () => {
		// Spy on Element.prototype so any element's scrollIntoView is captured,
		// regardless of which actual row node the listener picks. jsdom does
		// not implement scrollIntoView natively, so we also need to ensure it
		// exists on the prototype.
		if (typeof Element.prototype.scrollIntoView !== "function") {
			// eslint-disable-next-line @typescript-eslint/no-empty-function
			(Element.prototype as unknown as { scrollIntoView: () => void }).scrollIntoView = () => {};
		}
		const spy = vi.spyOn(Element.prototype, "scrollIntoView");

		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionList, { target, props: {} });
		flushSync();

		// Sanity: the row we expect to scroll actually rendered.
		const row = target.querySelector('[data-session-id="123"]');
		expect(row, "session row should render").not.toBeNull();

		window.dispatchEvent(
			new CustomEvent("tend:session-scroll-to", {
				detail: { sessionId: 123 },
			}),
		);

		expect(spy).toHaveBeenCalledTimes(1);
		expect(spy).toHaveBeenCalledWith({
			behavior: "smooth",
			block: "nearest",
		});
	});

	it("does nothing when the target session id is not in the list", () => {
		if (typeof Element.prototype.scrollIntoView !== "function") {
			(Element.prototype as unknown as { scrollIntoView: () => void }).scrollIntoView = () => {};
		}
		const spy = vi.spyOn(Element.prototype, "scrollIntoView");

		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionList, { target, props: {} });

		window.dispatchEvent(
			new CustomEvent("tend:session-scroll-to", {
				detail: { sessionId: 999_999 },
			}),
		);

		expect(spy).not.toHaveBeenCalled();
	});
});

// ---------------------------------------------------------------------------
// P4-D: svelte-dnd-action zone attachment smoke test. Verifies that each
// project group becomes a dnd source zone and its rows are draggable.
// ---------------------------------------------------------------------------

describe("SessionList P4-D drag-source wiring", () => {
	beforeEach(() => {
		sessionsStore.add(makeSession({ id: 321, label: "dnd-row" }));
	});

	afterEach(() => {
		if (component) {
			unmount(component);
			component = null;
		}
		sessionsStore.remove(321);
		document.body.innerHTML = "";
	});

	it("marks the project-group-items container as a svelte-dnd-action zone", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionList, { target, props: {} });
		flushSync();

		const zone = target.querySelector<HTMLElement>(".project-group-items");
		expect(zone, "zone element should render").not.toBeNull();
		// svelte-dnd-action sets several attributes / properties on its
		// zone container; we assert any one as a smoke signal. The
		// `role="list"` attribute is set by its a11y layer by default.
		const role = zone!.getAttribute("role");
		expect(role === "list" || role === "listbox" || role === "group").toBe(true);
	});

	it("renders session rows as direct dnd-zone items", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionList, { target, props: {} });
		flushSync();

		const row = target.querySelector<HTMLElement>('[data-session-id="321"]');
		expect(row, "session row should render").not.toBeNull();
		// svelte-dnd-action's keyboard a11y layer decorates each direct
		// child of the zone with role="listitem" and tabindex="0". We
		// assert either marker as a smoke signal that the action attached.
		const role = row!.getAttribute("role");
		const tabindex = row!.getAttribute("tabindex");
		expect(
			role === "listitem" || role === "button" || tabindex === "0" || tabindex === "-1",
			`expected dnd decoration on row (role=${role}, tabindex=${tabindex})`,
		).toBe(true);
	});

	it("passes onOpenInSlot down to SessionRow so the ⊞ button appears", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionList, {
			target,
			props: { onOpenInSlot: () => {} },
		});
		flushSync();

		const btn = target.querySelector<HTMLButtonElement>(".open-in-slot-btn");
		expect(btn, "⊞ button should render when onOpenInSlot is set").not.toBeNull();
	});
});
