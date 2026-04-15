// Phase 2 review fix: verify hex colour validation at the DB→DOM boundary.
// `project.settings.color` is freeform JSON and gets interpolated into
// `style="--project-color: ..."`; CSS custom-property values bypass Svelte's
// attribute-text escaping. A strict `#rrggbb` validator is the only thing
// standing between a bad DB row and CSS injection.

import { describe, expect, it } from "vitest";
import { isValidHexColor } from "./projects";

describe("isValidHexColor", () => {
	it("accepts lowercase 6-digit hex", () => {
		expect(isValidHexColor("#60a5fa")).toBe(true);
	});

	it("accepts uppercase 6-digit hex", () => {
		expect(isValidHexColor("#FFF000")).toBe(true);
	});

	it("accepts mixed-case 6-digit hex", () => {
		expect(isValidHexColor("#aB12cD")).toBe(true);
	});

	it("rejects named CSS colours", () => {
		expect(isValidHexColor("red")).toBe(false);
	});

	it("rejects 3-digit hex shorthand", () => {
		expect(isValidHexColor("#abc")).toBe(false);
	});

	it("rejects too-long hex strings", () => {
		expect(isValidHexColor("#abcdefg")).toBe(false);
	});

	it("rejects hex with surrounding whitespace", () => {
		expect(isValidHexColor("  #123456  ")).toBe(false);
	});

	it("rejects hex with trailing CSS injection", () => {
		expect(isValidHexColor("#60a5fa; background: url(x)")).toBe(false);
	});

	it("rejects the empty string", () => {
		expect(isValidHexColor("")).toBe(false);
	});

	it("rejects null", () => {
		expect(isValidHexColor(null)).toBe(false);
	});

	it("rejects undefined", () => {
		expect(isValidHexColor(undefined)).toBe(false);
	});

	it("rejects non-string types", () => {
		expect(isValidHexColor(0x60a5fa)).toBe(false);
		expect(isValidHexColor({ color: "#60a5fa" })).toBe(false);
		expect(isValidHexColor(["#60a5fa"])).toBe(false);
		expect(isValidHexColor(true)).toBe(false);
	});

	it("rejects hex missing the leading hash", () => {
		expect(isValidHexColor("60a5fa")).toBe(false);
	});
});
