/**
 * T139: E2E — Session lifecycle.
 *
 * Registers a project, spawns a session, asserts it appears, activates
 * it, asserts split view mounts, ends the session, asserts "ended".
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { expect, test } from "./fixtures";
import {
	clickSessionRow,
	endSession,
	registerProject,
	spawnSession,
	waitForAppReady,
	waitForSessionRow,
} from "./helpers";

test.describe("Session Lifecycle", () => {
	test("session appears, activates with split view, and ends", async ({
		page,
	}) => {
		await page.goto("/");
		await waitForAppReady(page);

		// Step 1: Register a project
		const projectId = await registerProject(
			page,
			"/tmp/e2e-session-test",
			"Session Test",
		);
		expect(projectId).toBeGreaterThan(0);

		// Step 2: Spawn a session
		const sessionId = await spawnSession(page, projectId, "test-agent");
		expect(sessionId).toBeGreaterThan(0);

		// Step 3: Assert session appears in sidebar
		await waitForSessionRow(page, "test-agent");

		// Verify the status badge shows a live status
		const statusBadge = page.locator(
			'.session-row:has-text("test-agent") .badge',
		);
		await expect(statusBadge.first()).toBeVisible();

		// Step 4: Activate the session by clicking the row
		await clickSessionRow(page, "test-agent");

		// Step 5: Assert split view mounts (both panes visible)
		const splitView = page.locator(".split-view");
		await expect(splitView).toBeVisible({ timeout: 3_000 });

		const agentPane = page.locator(".agent-pane");
		await expect(agentPane).toBeVisible();

		const companionPane = page.locator(".companion-pane");
		await expect(companionPane).toBeVisible();

		// Step 6: End the session
		await endSession(page, sessionId);

		// Step 7: SessionList.svelte hides ended sessions by default
		// (see src/lib/components/SessionList.svelte:95-99). Assert the
		// row vanishes from the active list rather than hunting for a
		// `.status-ended` badge that won't render.
		const sessionRow = page.locator(
			'.session-row:has(.session-label:has-text("test-agent"))',
		);
		await expect(sessionRow).toHaveCount(0, { timeout: 5_000 });
	});
});
