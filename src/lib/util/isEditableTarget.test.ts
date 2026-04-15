// P1-C: verify the single-key-shortcut guard correctly classifies text-input
// contexts. The xterm helper textarea check is critical — without it the `/`
// filter shortcut would hijack keystrokes intended for the embedded terminal.

import { describe, expect, it } from "vitest";
import { isEditableTarget } from "./isEditableTarget";

describe("isEditableTarget", () => {
	it("returns false for null", () => {
		expect(isEditableTarget(null)).toBe(false);
	});

	it("returns false for non-Element targets (e.g. document)", () => {
		expect(isEditableTarget(document as unknown as EventTarget)).toBe(false);
	});

	it("returns true for <input> elements", () => {
		const input = document.createElement("input");
		expect(isEditableTarget(input)).toBe(true);
	});

	it("returns true for <textarea> elements", () => {
		const ta = document.createElement("textarea");
		expect(isEditableTarget(ta)).toBe(true);
	});

	it("returns true for contenteditable elements", () => {
		const div = document.createElement("div");
		div.setAttribute("contenteditable", "true");
		// jsdom respects contenteditable via the isContentEditable property.
		document.body.append(div);
		try {
			expect(isEditableTarget(div)).toBe(true);
		} finally {
			div.remove();
		}
	});

	it("returns false for non-editable elements like <div>, <button>", () => {
		const div = document.createElement("div");
		const btn = document.createElement("button");
		expect(isEditableTarget(div)).toBe(false);
		expect(isEditableTarget(btn)).toBe(false);
	});

	it("returns true for elements inside .xterm-helper-textarea (xterm hidden textarea)", () => {
		// xterm renders a hidden <textarea class="xterm-helper-textarea"> inside
		// its terminal container to capture keystrokes. We must treat anything
		// inside that subtree as editable.
		const container = document.createElement("div");
		const helper = document.createElement("textarea");
		helper.className = "xterm-helper-textarea";
		container.append(helper);
		document.body.append(container);
		try {
			expect(isEditableTarget(helper)).toBe(true);
		} finally {
			container.remove();
		}
	});

	it("returns true for descendants of a .xterm-helper-textarea ancestor", () => {
		// Defensive: `.closest()` matches ancestors including the element itself,
		// so a child span should also be treated as editable.
		const wrapper = document.createElement("div");
		wrapper.className = "xterm-helper-textarea";
		const child = document.createElement("span");
		wrapper.append(child);
		document.body.append(wrapper);
		try {
			expect(isEditableTarget(child)).toBe(true);
		} finally {
			wrapper.remove();
		}
	});
});
