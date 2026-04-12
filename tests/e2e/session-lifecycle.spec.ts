/**
 * T139: E2E — Session lifecycle.
 *
 * Registers a project, simulates a daemon-IPC register_session,
 * asserts the session appears, activates it, asserts split view mounts,
 * ends the session, asserts it transitions to "ended".
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { test, expect } from '@playwright/test';
import {
  waitForAppReady,
  registerProject,
  simulateSessionRegister,
  waitForSessionRow,
  activateSession,
} from './helpers';

test.describe('Session Lifecycle', () => {
  test('session appears, activates with split view, and ends', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    // Step 1: Register a project
    const projectId = await registerProject(page, '/tmp/e2e-session-test', 'Session Test');
    expect(projectId).toBeGreaterThan(0);

    // Step 2: Spawn a session via Tauri invoke
    const sessionId = await simulateSessionRegister(page, projectId, 'test-agent');
    expect(sessionId).toBeGreaterThan(0);

    // Step 3: Assert session appears in sidebar
    await waitForSessionRow(page, 'test-agent');

    // Verify the status badge shows a live status
    const statusBadge = page.locator('.session-row:has-text("test-agent") .badge');
    await expect(statusBadge.first()).toBeVisible();

    // Step 4: Activate the session by clicking
    await activateSession(page, 'test-agent');

    // Step 5: Assert split view mounts (both panes visible)
    const splitView = page.locator('.split-view');
    await expect(splitView).toBeVisible({ timeout: 3_000 });

    const agentPane = page.locator('.agent-pane');
    await expect(agentPane).toBeVisible();

    const companionPane = page.locator('.companion-pane');
    await expect(companionPane).toBeVisible();

    // Step 6: End the session via invoke
    await page.evaluate(async (sid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('session_end', { sessionId: sid });
    }, sessionId);

    // Step 7: Assert session transitions to "ended"
    const endedBadge = page.locator(
      '.session-row:has-text("test-agent") .status-ended',
    );
    await expect(endedBadge).toBeVisible({ timeout: 5_000 });
  });
});
