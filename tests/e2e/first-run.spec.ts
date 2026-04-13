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

import { expect, test } from "@playwright/test";
import { registerProject, waitForAppReady } from "./helpers";

test.describe("First Run", () => {
	test("shows empty state on fresh launch", async ({ page }) => {
		await page.goto("http://localhost:1420");
		await waitForAppReady(page);

		const emptyState = page.locator("text=No projects registered");
		await expect(emptyState).toBeVisible({ timeout: 5_000 });
	});

	test("can register a project and see it in sidebar", async ({ page }) => {
		await page.goto("http://localhost:1420");
		await waitForAppReady(page);

		const projectId = await registerProject(
			page,
			"/tmp/e2e-test-project",
			"E2E Test",
		);
		expect(projectId).toBeGreaterThan(0);

		// Wait for the sidebar to update with the new project
		await page.waitForSelector("text=E2E Test", {
			state: "visible",
			timeout: 5_000,
		});
	});
});
