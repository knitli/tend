// P3-A: HamburgerButton is a tiny stateless toggle trigger. The component owns
// no state — it simply reflects the parent's `open` prop via aria-expanded and
// forwards clicks as `onToggle(!open)`. These tests assert the contract.

import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import HamburgerButton from "./HamburgerButton.svelte";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
});

describe("HamburgerButton", () => {
	it("renders with aria-expanded mirroring the open prop (open)", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(HamburgerButton, {
			target,
			props: {
				open: true,
				controlsId: "my-region",
				onToggle: () => {},
			},
		});

		const btn = target.querySelector<HTMLButtonElement>("button.hamburger");
		expect(btn).not.toBeNull();
		expect(btn!.getAttribute("aria-expanded")).toBe("true");
		expect(btn!.getAttribute("aria-controls")).toBe("my-region");
		expect(btn!.getAttribute("aria-label")).toBe("Collapse projects sidebar");
	});

	it("renders with aria-expanded=false when closed", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(HamburgerButton, {
			target,
			props: {
				open: false,
				controlsId: "my-region",
				onToggle: () => {},
			},
		});

		const btn = target.querySelector<HTMLButtonElement>("button.hamburger");
		expect(btn!.getAttribute("aria-expanded")).toBe("false");
		expect(btn!.getAttribute("aria-label")).toBe("Expand projects sidebar");
	});

	it("calls onToggle(!open) when clicked", () => {
		const onToggle = vi.fn();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(HamburgerButton, {
			target,
			props: {
				open: true,
				controlsId: "my-region",
				onToggle,
			},
		});

		const btn = target.querySelector<HTMLButtonElement>("button.hamburger");
		btn!.click();
		expect(onToggle).toHaveBeenCalledExactlyOnceWith(false);
	});

	it("calls onToggle(true) when clicked while closed", () => {
		const onToggle = vi.fn();
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(HamburgerButton, {
			target,
			props: {
				open: false,
				controlsId: "my-region",
				onToggle,
			},
		});

		const btn = target.querySelector<HTMLButtonElement>("button.hamburger");
		btn!.click();
		expect(onToggle).toHaveBeenCalledExactlyOnceWith(true);
	});
});
