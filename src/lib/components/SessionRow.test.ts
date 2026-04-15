// P1-A regression tests for the active-session indicator.
//
// Covers the behaviour introduced in Phase 1 of the adaptive UI: SessionRow
// receives an `active` prop that sets `.active` on the row root and renders
// the project-colour dot (now on `.session-row::before` per spec §8.1), and a
// stable `data-session-id` attribute is emitted so AlertBar's scroll-to can
// locate the row.
//
// Tests avoid the module-scoped shared ticker's background behaviour by
// unmounting after each case — the `refCount` pattern in SessionRow tears
// down the interval when the last instance unmounts.

import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import SessionRow from "./SessionRow.svelte";
import type { SessionSummary } from "$lib/api/sessions";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
});

function makeSession(overrides: Partial<SessionSummary> = {}): SessionSummary {
	return {
		id: 42,
		project_id: 1,
		label: "test-session",
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

describe("SessionRow active indicator", () => {
	it("applies the .active class when active=true", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionRow, {
			target,
			props: { session: makeSession(), active: true },
		});

		const row = target.querySelector<HTMLElement>(".session-row");
		expect(row).not.toBeNull();
		expect(row!.classList.contains("active")).toBe(true);
	});

	it("omits the .active class when active=false", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionRow, {
			target,
			props: { session: makeSession(), active: false },
		});

		const row = target.querySelector<HTMLElement>(".session-row");
		expect(row).not.toBeNull();
		expect(row!.classList.contains("active")).toBe(false);
	});

	it("sets data-session-id so AlertBar scroll-to can locate the row", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionRow, {
			target,
			props: { session: makeSession({ id: 7 }) },
		});

		const row = target.querySelector<HTMLElement>(".session-row");
		expect(row).not.toBeNull();
		expect(row!.getAttribute("data-session-id")).toBe("7");
	});

	it("dims the main text when anyActive=true and this row is not active", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionRow, {
			target,
			props: { session: makeSession(), active: false, anyActive: true },
		});

		const row = target.querySelector<HTMLElement>(".session-row");
		expect(row!.classList.contains("dimmed")).toBe(true);
	});

	// Phase 2-C: when a project has a settings.color, the row receives
	// `--project-color` as an inline style so the `.active` border + dot
	// use the real project colour rather than the global accent fallback.
	it("applies the --project-color inline style when projectColor is set", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionRow, {
			target,
			props: { session: makeSession(), projectColor: "#a78bfa" },
		});

		const row = target.querySelector<HTMLElement>(".session-row");
		expect(row).not.toBeNull();
		// The inline style is serialised to `--project-color: #a78bfa` on
		// the element — check via getAttribute rather than style.getPropertyValue
		// because JSDOM normalises CSS custom properties differently across versions.
		expect(row!.getAttribute("style")).toContain("--project-color");
		expect(row!.getAttribute("style")).toContain("#a78bfa");
	});

	it("omits the inline style when projectColor is null", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SessionRow, {
			target,
			props: { session: makeSession(), projectColor: null },
		});

		const row = target.querySelector<HTMLElement>(".session-row");
		// No inline style attribute → the component falls back to the
		// global `--color-accent` via CSS `var()` default.
		const style = row!.getAttribute("style");
		expect(style === null || !style.includes("--project-color")).toBe(true);
	});
});
