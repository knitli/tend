// P1-D: verify that SpinnerIcon renders with the expected DOM shape and honors
// accessible attributes. The motion-reduction fallback is implemented purely in
// CSS (@media prefers-reduced-motion), so we only assert the element structure
// here — not the applied animation.

import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import SpinnerIcon from "./SpinnerIcon.svelte";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
});

describe("SpinnerIcon", () => {
	it("renders a spinner element with the default size and label", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SpinnerIcon, { target });

		const el = target.querySelector<HTMLElement>(".spinner-icon");
		expect(el).not.toBeNull();
		expect(el!.getAttribute("role")).toBe("status");
		expect(el!.getAttribute("aria-label")).toBe("Loading");
		// Default size is 14 px — communicated via the --spinner-size custom prop.
		expect(el!.style.getPropertyValue("--spinner-size")).toBe("14px");
	});

	it("accepts a custom size prop", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SpinnerIcon, { target, props: { size: 24 } });

		const el = target.querySelector<HTMLElement>(".spinner-icon");
		expect(el).not.toBeNull();
		expect(el!.style.getPropertyValue("--spinner-size")).toBe("24px");
	});

	it("accepts a custom accessible label", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(SpinnerIcon, {
			target,
			props: { label: "Refreshing layouts" },
		});

		const el = target.querySelector<HTMLElement>(".spinner-icon");
		expect(el!.getAttribute("aria-label")).toBe("Refreshing layouts");
	});
});
