/**
 * T139: E2E — Session lifecycle.
 *
 * Registers a project, spawns a session, asserts it appears, activates
 * it, asserts split view mounts, ends the session, asserts "ended".
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { test, expect } from '@playwright/test';
import {
  waitForAppReady,
  registerProject,
  spawnSession,
  endSession,
  waitForSessionRow,
  clickSessionRow,
} from './helpers';

test.describe('Session Lifecycle', () => {
  test('session appears, activates with split view, and ends', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    // Step 1: Register a project
    const projectId = await registerProject(page, '/tmp/e2e-session-test', 'Session Test');
    expect(projectId).toBeGreaterThan(0);

    // Step 2: Spawn a session
    const sessionId = await spawnSession(page, projectId, 'test-agent');
    expect(sessionId).toBeGreaterThan(0);

    // Step 3: Assert session appears in sidebar
    await waitForSessionRow(page, 'test-agent');

    // Verify the status badge shows a live status
    const statusBadge = page.locator('.session-row:has-text("test-agent") .badge');
    await expect(statusBadge.first()).toBeVisible();

    // Step 4: Activate the session by clicking the row
    await clickSessionRow(page, 'test-agent');

    // Step 5: Assert split view mounts (both panes visible)
    const splitView = page.locator('.split-view');
    await expect(splitView).toBeVisible({ timeout: 3_000 });

    const agentPane = page.locator('.agent-pane');
    await expect(agentPane).toBeVisible();

    const companionPane = page.locator('.companion-pane');
    await expect(companionPane).toBeVisible();

    // Step 6: End the session
    await endSession(page, sessionId);

    // Step 7: Assert session transitions to "ended"
    const endedBadge = page.locator(
      '.session-row:has-text("test-agent") .status-ended',
    );
    await expect(endedBadge).toBeVisible({ timeout: 5_000 });
  });
});
