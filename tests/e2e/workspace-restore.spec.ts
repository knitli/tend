/**
 * T141: E2E — Workspace state restore.
 *
 * Saves workspace state, reloads, asserts state restored.
 * Saves a named layout, lists it, restores it.
 */

import { expect, test } from "./fixtures";
import { invoke, registerProject, waitForAppReady } from "./helpers";

test.describe("Workspace Restore", () => {
	test("workspace state persists across reload", async ({ page }) => {
		await page.goto("/");
		await waitForAppReady(page);

		await registerProject(page, "/tmp/e2e-workspace", "Workspace Test");

		await invoke(page, "workspace_save", {
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

		await page.reload();
		await waitForAppReady(page);

		const restored = await invoke<{
			state: { version: number; pane_layout: string };
		}>(page, "workspace_get");

		expect(restored.state.version).toBe(1);
		expect(restored.state.pane_layout).toBe("split");
	});

	test("named layout save and restore", async ({ page }) => {
		await page.goto("/");
		await waitForAppReady(page);

		const saveResult = await invoke<{
			layout: { id: number; name: string };
		}>(page, "layout_save", {
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
		expect(saveResult.layout.name).toBe("E2E Layout");

		const layouts = await invoke<{
			layouts: Array<{ id: number; name: string }>;
		}>(page, "layout_list");
		expect(layouts.layouts.some((l) => l.name === "E2E Layout")).toBe(true);

		const layout = layouts.layouts.find((l) => l.name === "E2E Layout");
		if (!layout) throw new Error("Layout not found");

		const restored = await invoke<{
			state: { pane_layout: string };
			missing_sessions: number[];
		}>(page, "layout_restore", { args: { id: layout.id } });

		expect(restored.state.pane_layout).toBe("split");
		expect(restored.missing_sessions).toEqual([]);
	});
});
