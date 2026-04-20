import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import NavRail, { type NavId } from "./NavRail.svelte";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
	vi.restoreAllMocks();
});

describe("NavRail", () => {
	it("emits onChange for each nav item", () => {
		const onChange = vi.fn();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(NavRail, {
			target,
			props: {
				value: "workspaces",
				onChange,
				open: true,
			},
		});

		const buttons = Array.from(
			target.querySelectorAll<HTMLButtonElement>("button.nav-item"),
		);
		const expected: NavId[] = [
			"workspaces",
			"dashboard",
			"sessions",
			"projects",
			"settings",
		];

		expect(buttons).toHaveLength(expected.length);
		buttons.forEach((button) => button.click());
		expect(onChange.mock.calls.map(([value]) => value)).toEqual(expected);
	});

	it("sets aria-current only for the active item", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(NavRail, {
			target,
			props: {
				value: "sessions",
				onChange: () => {},
				open: true,
			},
		});

		const current = target.querySelector<HTMLButtonElement>(
			"button.nav-item[aria-current='page']",
		);
		expect(current?.textContent).toContain("Sessions");
		const nonCurrent = target.querySelectorAll<HTMLButtonElement>(
			"button.nav-item:not([aria-current])",
		);
		expect(nonCurrent).toHaveLength(4);
	});

	it("wires collapse and expand toggle through onToggle", () => {
		const collapseSpy = vi.fn();
		const collapseTarget = document.createElement("div");
		document.body.append(collapseTarget);
		component = mount(NavRail, {
			target: collapseTarget,
			props: {
				value: "workspaces",
				onChange: () => {},
				open: true,
				onToggle: collapseSpy,
			},
		});

		const collapseButton = collapseTarget.querySelector<HTMLButtonElement>(
			"button.nav-edge-toggle",
		);
		expect(collapseButton?.getAttribute("aria-expanded")).toBe("true");
		collapseButton?.click();
		expect(collapseSpy).toHaveBeenCalledExactlyOnceWith(false);

		unmount(component);
		component = null;

		const expandSpy = vi.fn();
		const expandTarget = document.createElement("div");
		document.body.append(expandTarget);
		component = mount(NavRail, {
			target: expandTarget,
			props: {
				value: "workspaces",
				onChange: () => {},
				open: false,
				onToggle: expandSpy,
			},
		});

		const expandButton = expandTarget.querySelector<HTMLButtonElement>(
			"button.nav-edge-toggle",
		);
		expect(expandButton?.getAttribute("aria-expanded")).toBe("false");
		expandButton?.click();
		expect(expandSpy).toHaveBeenCalledExactlyOnceWith(true);
	});
});
