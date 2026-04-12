/**
 * T138: E2E — First run experience.
 *
 * Launches the app, asserts empty-state "No projects registered",
 * clicks "Add Project", selects a temp dir, confirms the project appears.
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { test, expect } from '@playwright/test';
import { waitForAppReady, registerProject } from './helpers';

test.describe('First Run', () => {
  test('shows empty state on fresh launch', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    // The sidebar should show the empty-state message
    const emptyState = page.locator('text=No projects registered');
    await expect(emptyState).toBeVisible({ timeout: 5_000 });
  });

  test('can register a project and see it in sidebar', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    // Register a project via invoke bridge (simulates the Add Project flow)
    const projectId = await registerProject(page, '/tmp/e2e-test-project', 'E2E Test');
    expect(projectId).toBeGreaterThan(0);

    // Wait for the sidebar to update
    await page.waitForSelector('text=E2E Test', {
      state: 'visible',
      timeout: 5_000,
    });

    // Verify project appears in the project list
    const projectEntry = page.locator('.project-entry:has-text("E2E Test")');
    await expect(projectEntry).toBeVisible();
  });
});
