/**
 * T140: E2E — Scratchpad persistence.
 *
 * Adds notes and reminders, reloads, verifies persistence,
 * queries cross-project overview, marks a reminder done.
 *
 * Requires: tauri-driver + built Tauri app.
 */

import { test, expect } from '@playwright/test';
import { waitForAppReady, registerProject } from './helpers';

test.describe('Scratchpad', () => {
  test('notes and reminders persist across reload', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    const projectId = await registerProject(page, '/tmp/e2e-scratchpad', 'Scratchpad Test');

    // Add a note via invoke
    const noteId = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ note: { id: number } }>('note_create', {
        args: { project_id: pid, content: 'E2E persistent note' },
      });
      return result.note.id;
    }, projectId);
    expect(noteId).toBeGreaterThan(0);

    // Add a reminder via invoke
    const reminderId = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ reminder: { id: number } }>('reminder_create', {
        args: { project_id: pid, content: 'E2E persistent reminder' },
      });
      return result.reminder.id;
    }, projectId);
    expect(reminderId).toBeGreaterThan(0);

    // Reload to simulate restart
    await page.reload();
    await waitForAppReady(page);

    // Verify note persists by querying backend
    const notes = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{ notes: Array<{ content: string }> }>('note_list', {
        args: { project_id: pid },
      });
      return result.notes;
    }, projectId);
    expect(notes.some((n) => n.content === 'E2E persistent note')).toBe(true);

    // Verify reminder persists
    const reminders = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{
        reminders: Array<{ content: string; state: string }>;
      }>('reminder_list', {
        args: { project_id: pid },
      });
      return result.reminders;
    }, projectId);
    expect(reminders.some((r) => r.content === 'E2E persistent reminder')).toBe(true);

    // Mark reminder done via invoke
    await page.evaluate(async (rid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('reminder_set_state', {
        args: { id: rid, state: 'done' },
      });
    }, reminderId);

    // Verify reminder state changed
    const doneReminders = await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<{
        reminders: Array<{ id: number; state: string }>;
      }>('reminder_list', {
        args: { project_id: pid, states: ['done'] },
      });
      return result.reminders;
    }, projectId);
    expect(doneReminders.some((r) => r.id === reminderId && r.state === 'done')).toBe(true);
  });

  test('cross-project overview shows reminders from multiple projects', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await waitForAppReady(page);

    const pid1 = await registerProject(page, '/tmp/e2e-scratch-1', 'Project Alpha');
    const pid2 = await registerProject(page, '/tmp/e2e-scratch-2', 'Project Beta');

    await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('reminder_create', {
        args: { project_id: pid, content: 'Alpha reminder' },
      });
    }, pid1);

    await page.evaluate(async (pid) => {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('reminder_create', {
        args: { project_id: pid, content: 'Beta reminder' },
      });
    }, pid2);

    // Query overview via invoke
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
