/**
 * T138: E2E — First run experience.
 *
 * Launches the app, asserts empty-state "No projects registered",
 * registers a project via invoke, confirms the project appears.
 *
 * Note: The "clicks Add Project" UI flow requires a native file dialog
 * which is not automatable via Playwright. We test the register path
 * via invoke instead and verify the UI updates.
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { expect, test } from "./fixtures";
import { registerProject, waitForAppReady } from "./helpers";

test.describe("First Run", () => {
	test("shows empty state on fresh launch", async ({ page }) => {
		await page.goto("/");
		await waitForAppReady(page);

		// Sidebar.svelte renders this exact copy when project list is empty.
		const emptyState = page.locator(
			"text=No active projects. Add one to get started.",
		);
		await expect(emptyState).toBeVisible({ timeout: 5_000 });
	});

	test("can register a project and see it in sidebar", async ({ page }) => {
		await page.goto("/");
		await waitForAppReady(page);

		const projectId = await registerProject(
			page,
			"/tmp/e2e-test-project",
			"E2E Test",
		);
		expect(projectId).toBeGreaterThan(0);

		// projectsStore.register is the UI path; backend-side invoke does not
		// push a project:added event (no such event exists). Reload so the
		// store re-hydrates from the (mock) backend and the row appears.
		await page.reload();
		await waitForAppReady(page);

		await page.waitForSelector("text=E2E Test", {
			state: "visible",
			timeout: 5_000,
		});
	});
});
