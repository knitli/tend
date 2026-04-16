/**
 * T140: E2E — Scratchpad persistence.
 *
 * Adds notes and reminders, reloads, verifies persistence,
 * queries cross-project overview, marks a reminder done.
 */

import { expect, test } from "./fixtures";
import { invoke, registerProject, waitForAppReady } from "./helpers";

test.describe("Scratchpad", () => {
	test("notes and reminders persist across reload", async ({ page }) => {
		await page.goto("/");
		await waitForAppReady(page);

		const projectId = await registerProject(
			page,
			"/tmp/e2e-scratchpad",
			"Scratchpad Test",
		);

		const noteRes = await invoke<{ note: { id: number } }>(
			page,
			"note_create",
			{ args: { project_id: projectId, content: "E2E persistent note" } },
		);
		expect(noteRes.note.id).toBeGreaterThan(0);

		const reminderRes = await invoke<{ reminder: { id: number } }>(
			page,
			"reminder_create",
			{
				args: {
					project_id: projectId,
					content: "E2E persistent reminder",
				},
			},
		);
		expect(reminderRes.reminder.id).toBeGreaterThan(0);
		const reminderId = reminderRes.reminder.id;

		// Reload to simulate restart. The mock persists state to sessionStorage,
		// so the same browser context (single test) sees data across reload.
		await page.reload();
		await waitForAppReady(page);

		const notes = await invoke<{ notes: Array<{ content: string }> }>(
			page,
			"note_list",
			{ args: { project_id: projectId } },
		);
		expect(notes.notes.some((n) => n.content === "E2E persistent note")).toBe(
			true,
		);

		const reminders = await invoke<{
			reminders: Array<{ content: string; state: string }>;
		}>(page, "reminder_list", { args: { project_id: projectId } });
		expect(
			reminders.reminders.some((r) => r.content === "E2E persistent reminder"),
		).toBe(true);

		await invoke(page, "reminder_set_state", {
			args: { id: reminderId, state: "done" },
		});

		const doneReminders = await invoke<{
			reminders: Array<{ id: number; state: string }>;
		}>(page, "reminder_list", {
			args: { project_id: projectId, states: ["done"] },
		});
		expect(
			doneReminders.reminders.some(
				(r) => r.id === reminderId && r.state === "done",
			),
		).toBe(true);
	});

	test("cross-project overview shows reminders from multiple projects", async ({
		page,
	}) => {
		await page.goto("/");
		await waitForAppReady(page);

		const pid1 = await registerProject(
			page,
			"/tmp/e2e-scratch-1",
			"Project Alpha",
		);
		const pid2 = await registerProject(
			page,
			"/tmp/e2e-scratch-2",
			"Project Beta",
		);

		await invoke(page, "reminder_create", {
			args: { project_id: pid1, content: "Alpha reminder" },
		});
		await invoke(page, "reminder_create", {
			args: { project_id: pid2, content: "Beta reminder" },
		});

		const overview = await invoke<{
			groups: Array<{ project_display_name: string }>;
		}>(page, "cross_project_overview");

		const projectNames = overview.groups.map((g) => g.project_display_name);
		expect(projectNames).toContain("Project Alpha");
		expect(projectNames).toContain("Project Beta");
	});
});
