// P4-F: MainTabs tests. MainTabs is a thin wrapper around bits-ui's Tabs
// primitives; the interesting contract is:
//   1. only the snippet for the currently-active tab is rendered (important
//      because the real callers put terminal-spawning components inside),
//   2. clicking a trigger fires onValueChange with the right TabId.
// Snippet props are created via `createRawSnippet`, which is the documented
// Svelte 5 API for synthesizing snippets outside of template syntax.

import { createRawSnippet, flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import MainTabs, { type TabId } from "./MainTabs.svelte";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
});

/** Build a raw snippet that renders a single <span data-test={marker}>. */
function markerSnippet(marker: string) {
	return createRawSnippet(() => ({
		render: () => `<span data-test="${marker}">${marker}</span>`,
	}));
}

describe("MainTabs", () => {
	it("renders exactly one active tab's content at a time (sessions)", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(MainTabs, {
			target,
			props: {
				value: "sessions",
				onValueChange: () => {},
				sessionsContent: markerSnippet("SESS"),
				workspaceContent: markerSnippet("WORK"),
				overviewContent: markerSnippet("OVER"),
			},
		});
		flushSync();

		// Only the sessions marker is actually rendered into the DOM; the
		// other two Tabs.Content regions exist (for bits-ui's ARIA wiring)
		// but their snippet bodies aren't invoked.
		expect(target.querySelector("[data-test='SESS']")).not.toBeNull();
		expect(target.querySelector("[data-test='WORK']")).toBeNull();
		expect(target.querySelector("[data-test='OVER']")).toBeNull();
	});

	it("switches the rendered content when `value` changes", () => {
		const target = document.createElement("div");
		document.body.append(target);
		let current: TabId = "sessions";
		component = mount(MainTabs, {
			target,
			props: {
				value: current,
				onValueChange: (v) => {
					current = v;
				},
				sessionsContent: markerSnippet("SESS"),
				workspaceContent: markerSnippet("WORK"),
				overviewContent: markerSnippet("OVER"),
			},
		});
		flushSync();

		// Find the "Workspace" trigger and click it. bits-ui's default
		// activationMode is 'automatic', so focus alone would activate, but
		// we simulate a click for determinism across environments.
		const triggers = Array.from(
			target.querySelectorAll<HTMLButtonElement>("button.main-tabs-trigger"),
		);
		expect(triggers.map((t) => t.textContent?.trim())).toEqual([
			"Sessions",
			"Workspace",
			"Overview",
		]);

		const workspaceTrigger = triggers.find(
			(t) => t.textContent?.trim() === "Workspace",
		)!;
		workspaceTrigger.click();
		flushSync();

		// The parent's onValueChange callback is responsible for re-assigning
		// the `value` prop. In this harness we only captured the new id into
		// `current`; assert that's right, then verify bits-ui itself flipped
		// the DOM state via data-state on the trigger.
		expect(current).toBe("workspace");
		expect(workspaceTrigger.getAttribute("data-state")).toBe("active");
	});

	it("fires onValueChange with the clicked tab's id", () => {
		const onValueChange = vi.fn<(v: TabId) => void>();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(MainTabs, {
			target,
			props: {
				value: "sessions",
				onValueChange,
				sessionsContent: markerSnippet("SESS"),
				workspaceContent: markerSnippet("WORK"),
				overviewContent: markerSnippet("OVER"),
			},
		});
		flushSync();

		const overviewTrigger = Array.from(
			target.querySelectorAll<HTMLButtonElement>("button.main-tabs-trigger"),
		).find((t) => t.textContent?.trim() === "Overview")!;
		overviewTrigger.click();
		flushSync();

		expect(onValueChange).toHaveBeenCalledWith("overview");
	});
});
