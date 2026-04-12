/**
 * T140: E2E — Scratchpad persistence.
 *
 * Adds notes and reminders, relaunches (simulated via store re-hydration),
 * verifies persistence, opens cross-project overview, marks a reminder done.
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { test, expect } from '@playwright/test';
import { waitForAppReady, registerProject } from './helpers';

test.describe('Scratchpad', () => {
  test('notes and reminders persist across re-hydration', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    // Register a project
    const projectId = await registerProject(page, '/tmp/e2e-scratchpad', 'Scratchpad Test');

    // Add a note via invoke
    const noteId = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ id: number }>('note_create', {
        projectId: pid,
        content: 'E2E persistent note',
      });
      return result.id;
    }, projectId);
    expect(noteId).toBeGreaterThan(0);

    // Add a reminder via invoke
    const reminderId = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ id: number }>('reminder_create', {
        projectId: pid,
        content: 'E2E persistent reminder',
      });
      return result.id;
    }, projectId);
    expect(reminderId).toBeGreaterThan(0);

    // Simulate "relaunch" by re-hydrating the stores
    await page.evaluate(async () => {
      // Force a full page reload to simulate restart
      window.location.reload();
    });
    await waitForAppReady(page);

    // Verify note persists by querying backend
    const notes = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ notes: Array<{ content: string }> }>('note_list', {
        projectId: pid,
      });
      return result.notes;
    }, projectId);
    expect(notes.some((n) => n.content === 'E2E persistent note')).toBe(true);

    // Verify reminder persists
    const reminders = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ reminders: Array<{ content: string; state: string }> }>(
        'reminder_list',
        { projectId: pid },
      );
      return result.reminders;
    }, projectId);
    expect(reminders.some((r) => r.content === 'E2E persistent reminder')).toBe(true);

    // Mark reminder done via invoke
    await page.evaluate(async (rid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('reminder_set_state', { reminderId: rid, state: 'done' });
    }, reminderId);

    // Verify reminder state changed
    const updatedReminders = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ reminders: Array<{ id: number; state: string }> }>(
        'reminder_list',
        { projectId: pid, states: ['done'] },
      );
      return result.reminders;
    }, projectId);
    expect(updatedReminders.some((r) => r.id === reminderId && r.state === 'done')).toBe(true);
  });

  test('cross-project overview shows reminders from multiple projects', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    // Register two projects
    const pid1 = await registerProject(page, '/tmp/e2e-scratch-1', 'Project Alpha');
    const pid2 = await registerProject(page, '/tmp/e2e-scratch-2', 'Project Beta');

    // Add reminders to each
    await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('reminder_create', { projectId: pid, content: 'Alpha reminder' });
    }, pid1);

    await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('reminder_create', { projectId: pid, content: 'Beta reminder' });
    }, pid2);

    // Query overview
    const overview = await page.evaluate(async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      return invoke<{ groups: Array<{ project_display_name: string }> }>(
        'cross_project_overview',
      );
    });

    const projectNames = overview.groups.map((g) => g.project_display_name);
    expect(projectNames).toContain('Project Alpha');
    expect(projectNames).toContain('Project Beta');
  });
});
