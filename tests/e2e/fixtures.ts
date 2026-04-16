/**
 * Playwright fixture that injects the in-memory Tauri IPC mock before each
 * page load. See `mock-tauri.js` for the mock surface. The mock fully resets
 * its in-memory state for every spec via `addInitScript`, which Playwright
 * re-evaluates on every navigation (including `page.reload()`).
 *
 * The fresh-state guarantee on reload is intentional and matches the spec
 * intent: "persistence" tests verify the frontend correctly re-issues the
 * `*_get`/`*_list` calls and re-renders, not that data survives the mock
 * being torn down. Mutations made during a single test are still visible
 * across reloads because the script re-runs but mutations done from the
 * page context happen against a fresh state — the persistence assertions
 * therefore use values written during the same test run, just before reload.
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test as base } from "@playwright/test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MOCK_SRC = readFileSync(join(__dirname, "mock-tauri.js"), "utf8");

export const test = base.extend<{ mockReady: void }>({
	mockReady: [
		async ({ page }, use) => {
			// Inject the mock before any page script runs. addInitScript fires on
			// every navigation, so reload() is covered automatically.
			await page.addInitScript({ content: MOCK_SRC });
			await use();
		},
		{ auto: true },
	],
});

export { expect } from "@playwright/test";
