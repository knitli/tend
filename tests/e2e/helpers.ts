/**
 * Shared E2E test helpers for Tauri app testing via tauri-driver.
 *
 * These helpers abstract the Tauri WebDriver connection and provide
 * utilities for common operations like project registration and
 * session simulation.
 */

import { type Page } from '@playwright/test';

/**
 * Wait for the Tauri app to be fully loaded.
 * Checks for the root Svelte mount point and initial hydration.
 */
export async function waitForAppReady(page: Page): Promise<void> {
  // Wait for the Svelte app to mount
  await page.waitForSelector('#app', { state: 'attached', timeout: 10_000 });
  // Wait for initial store hydration (loading state clears)
  await page.waitForFunction(
    () => !document.querySelector('[data-loading="true"]'),
    { timeout: 5_000 },
  ).catch(() => {
    // Loading indicator may not exist; that's fine
  });
}

/**
 * Register a project via the Tauri invoke bridge.
 * Returns the project id.
 */
export async function registerProject(
  page: Page,
  path: string,
  name: string,
): Promise<number> {
  return page.evaluate(
    async ([p, n]) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ id: number }>('project_register', {
        path: p,
        displayName: n,
      });
      return result.id;
    },
    [path, name] as const,
  );
}

/**
 * Simulate a daemon IPC register_session by invoking the Tauri command directly.
 * This bypasses the actual CLI wrapper for test purposes.
 */
export async function simulateSessionRegister(
  page: Page,
  projectId: number,
  label: string,
): Promise<number> {
  return page.evaluate(
    async ([pid, lbl]) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ id: number }>('session_spawn', {
        projectId: pid,
        label: lbl,
        command: ['/bin/sh', '-c', 'sleep 3600'],
        workingDirectory: '/tmp',
        env: {},
      });
      return result.id;
    },
    [projectId, label] as const,
  );
}

/**
 * Get the text content of an element matching the selector.
 */
export async function getText(page: Page, selector: string): Promise<string> {
  const el = page.locator(selector);
  return el.textContent() ?? '';
}

/**
 * Wait for a session row to appear in the sidebar.
 */
export async function waitForSessionRow(
  page: Page,
  label: string,
  timeout = 5_000,
): Promise<void> {
  await page.waitForSelector(`.session-label:has-text("${label}")`, {
    state: 'visible',
    timeout,
  });
}

/**
 * Click a session row by label to activate it.
 */
export async function activateSession(
  page: Page,
  label: string,
): Promise<void> {
  await page.click(`.session-row:has(.session-label:has-text("${label}"))`);
}
