/**
 * T141: E2E — Workspace state restore.
 *
 * Saves workspace state, reloads, asserts state restored.
 * Saves a named layout, lists it, restores it.
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { expect, test } from "@playwright/test";
import { registerProject, waitForAppReady } from "./helpers";

test.describe("Workspace Restore", () => {
	test("workspace state persists across reload", async ({ page }) => {
		await page.goto("http://localhost:1420");
		await waitForAppReady(page);

		await registerProject(page, "/tmp/e2e-workspace", "Workspace Test");

		// Save workspace state via invoke
		await page.evaluate(async () => {
			const { invoke } = await import("@tauri-apps/api/core");
			await invoke("workspace_save", {
				args: {
					state: {
						version: 1,
						active_project_ids: [],
						focused_session_id: null,
						pane_layout: "split",
						ui: { zoom: 1.0 },
					},
				},
			});
		});

		// Reload to simulate restart
		await page.reload();
		await waitForAppReady(page);

		// Verify workspace state was restored
		const restored = await page.evaluate(async () => {
			const { invoke } = await import("@tauri-apps/api/core");
			return invoke<{
				state: { version: number; pane_layout: string };
			}>("workspace_get");
		});

		expect(restored.state.version).toBe(1);
		expect(restored.state.pane_layout).toBe("split");
	});

	test("named layout save and restore", async ({ page }) => {
		await page.goto("http://localhost:1420");
		await waitForAppReady(page);

		// Save a named layout
		const saveResult = await page.evaluate(async () => {
			const { invoke } = await import("@tauri-apps/api/core");
			return invoke<{ layout: { id: number; name: string } }>("layout_save", {
				args: {
					name: "E2E Layout",
					state: {
						version: 1,
						active_project_ids: [],
						focused_session_id: null,
						pane_layout: "split",
						ui: {},
					},
					overwrite: false,
				},
			});
		});
		expect(saveResult.layout.name).toBe("E2E Layout");

		// List layouts and verify ours exists
		const layouts = await page.evaluate(async () => {
			const { invoke } = await import("@tauri-apps/api/core");
			return invoke<{ layouts: Array<{ id: number; name: string }> }>(
				"layout_list",
			);
		});
		expect(layouts.layouts.some((l) => l.name === "E2E Layout")).toBe(true);

		// Restore the layout
		const layout = layouts.layouts.find((l) => l.name === "E2E Layout");
		if (!layout) throw new Error("Layout not found");
		const restored = await page.evaluate(async (lid) => {
			const { invoke } = await import("@tauri-apps/api/core");
			return invoke<{
				state: { pane_layout: string };
				missing_sessions: number[];
			}>("layout_restore", { args: { id: lid } });
		}, layout.id);

		expect(restored.state.pane_layout).toBe("split");
		// missing_sessions should be empty since no sessions were referenced
		expect(restored.missing_sessions).toEqual([]);
	});
});
