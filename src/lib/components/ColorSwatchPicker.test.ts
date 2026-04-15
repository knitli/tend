// Phase 2-B smoke tests for the project colour picker popover.
//
// We can't exercise the full interactive behaviour of the `<hex-color-picker>`
// custom element in JSDOM (it relies on pointer events and layout), but we
// can assert that:
//   1. The component mounts successfully.
//   2. The custom element is registered lazily and appears in the DOM.
//   3. The `color-changed` custom event fired on the picker element causes
//      `onChange` to be invoked with the new hex value.
//   4. Pressing Escape inside the popover calls `onClose`.
//
// The dynamic import of `vanilla-colorful/hex-color-picker.js` is async —
// we flush microtasks + a macrotask before asserting the element is present.

import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import ColorSwatchPicker from "./ColorSwatchPicker.svelte";

let component: ReturnType<typeof mount> | null = null;

afterEach(() => {
	if (component) {
		unmount(component);
		component = null;
	}
	document.body.innerHTML = "";
});

/** Wait until the `<hex-color-picker>` element is present in `target`,
 *  polling up to ~500 ms. The custom-element registration happens inside a
 *  `.then(...)` continuation from the dynamic import, and the effect that
 *  flips `ready = true` only runs on the next Svelte tick after that. */
async function waitForPicker(target: HTMLElement): Promise<Element | null> {
	const deadline = Date.now() + 500;
	while (Date.now() < deadline) {
		const el = target.querySelector("hex-color-picker");
		if (el) return el;
		await new Promise((r) => setTimeout(r, 10));
	}
	return null;
}

describe("ColorSwatchPicker", () => {
	it("mounts and renders the hex-color-picker custom element", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(ColorSwatchPicker, {
			target,
			props: {
				value: "#60a5fa",
				onChange: vi.fn(),
				onClose: vi.fn(),
			},
		});

		const picker = await waitForPicker(target);
		expect(picker).not.toBeNull();
		expect(target.querySelector(".color-swatch-picker")).not.toBeNull();
	});

	it("renders the popover root even before the custom element loads", () => {
		const target = document.createElement("div");
		document.body.append(target);
		component = mount(ColorSwatchPicker, {
			target,
			props: {
				value: "#60a5fa",
				onChange: vi.fn(),
				onClose: vi.fn(),
			},
		});

		// The `.color-swatch-picker` root is synchronous; only the inner
		// `<hex-color-picker>` is gated on the dynamic import.
		expect(target.querySelector(".color-swatch-picker")).not.toBeNull();
		expect(target.querySelector(".color-swatch-picker-value")).not.toBeNull();
	});

	it("calls onChange with the hex from a color-changed event", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		const onChange = vi.fn();

		component = mount(ColorSwatchPicker, {
			target,
			props: {
				value: "#60a5fa",
				onChange,
				onClose: vi.fn(),
			},
		});

		const picker = await waitForPicker(target);
		expect(picker).not.toBeNull();

		// Dispatch the same event shape that vanilla-colorful emits internally.
		picker!.dispatchEvent(
			new CustomEvent("color-changed", {
				detail: { value: "#34d399" },
				bubbles: true,
			}),
		);

		expect(onChange).toHaveBeenCalledWith("#34d399");
	});

	it("calls onClose when Escape is pressed (document keydown)", async () => {
		const target = document.createElement("div");
		document.body.append(target);
		const onClose = vi.fn();

		component = mount(ColorSwatchPicker, {
			target,
			props: {
				value: "#60a5fa",
				onChange: vi.fn(),
				onClose,
			},
		});

		// Allow onMount to complete so the document keydown listener is registered.
		await new Promise((r) => setTimeout(r, 0));

		// Dispatch at the document level — the listener is now registered there
		// (capture phase) so that Escape closes the picker regardless of where
		// focus sits (usually the swatch button, not the popover itself).
		document.dispatchEvent(
			new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
		);

		expect(onClose).toHaveBeenCalledTimes(1);
	});
});
