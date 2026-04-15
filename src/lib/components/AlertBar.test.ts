// P1-B regression test: AlertBar's "Go to" button must
//   1. invoke the `onActivateSession` callback for the right session, and
//   2. dispatch a `tend:session-scroll-to` CustomEvent on the window with
//      `{ sessionId }` so SessionList can scroll the row into view.

import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import AlertBar from "./AlertBar.svelte";
import { sessionsStore } from "$lib/stores/sessions.svelte";
import { projectsStore } from "$lib/stores/projects.svelte";
import type { SessionSummary } from "$lib/api/sessions";

let component: ReturnType<typeof mount> | null = null;

function makeSessionWithAlert(
	overrides: Partial<SessionSummary> = {},
): SessionSummary {
	return {
		id: 99,
		project_id: 1,
		label: "alerted-session",
		pid: 1234,
		status: "needs_input",
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
		alert: {
			id: 1,
			session_id: 99,
			project_id: 1,
			reason: "Waiting for input",
			raised_at: "2026-04-15T12:00:01Z",
		},
		reattached_mirror: false,
		...overrides,
	};
}

beforeEach(() => {
	// AlertBar reads sessions from the module-scoped store. Seed it with a
	// single session that has an alert, so AlertBar renders one row.
	sessionsStore.add(makeSessionWithAlert());
	// projectsStore.byId returning undefined is fine for this test (AlertBar
	// falls back to "Project #<id>"), so no project seeding needed.
});

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	// Clean up the seeded session to avoid cross-test pollution.
	sessionsStore.remove(99);
	// Reset projects store (best effort — the store exposes hydrate but we
	// didn't seed it, so nothing to undo).
	void projectsStore;
	document.body.innerHTML = "";
	vi.restoreAllMocks();
});

describe("AlertBar 'Go to' button", () => {
	it("invokes onActivateSession and dispatches tend:session-scroll-to", () => {
		const onActivateSession = vi.fn();
		const scrollListener = vi.fn();
		window.addEventListener("tend:session-scroll-to", scrollListener);

		const target = document.createElement("div");
		document.body.append(target);
		component = mount(AlertBar, {
			target,
			props: { onActivateSession },
		});
		flushSync();

		const goBtn =
			target.querySelector<HTMLButtonElement>(".alert-go-btn");
		expect(goBtn, "Go to button should be rendered").not.toBeNull();
		goBtn!.click();

		expect(onActivateSession).toHaveBeenCalledTimes(1);
		expect(onActivateSession).toHaveBeenCalledWith(
			expect.objectContaining({ id: 99 }),
		);

		expect(scrollListener).toHaveBeenCalledTimes(1);
		const ev = scrollListener.mock.calls[0][0] as CustomEvent<{
			sessionId: number;
		}>;
		expect(ev.type).toBe("tend:session-scroll-to");
		expect(ev.detail).toEqual({ sessionId: 99 });

		window.removeEventListener("tend:session-scroll-to", scrollListener);
	});
});
