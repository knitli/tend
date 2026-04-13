/**
 * Shared E2E test helpers for Tauri app testing via tauri-driver.
 *
 * All invoke calls use the { args: { snake_case } } pattern matching
 * the Rust #[tauri::command] arg struct convention.
 */

import type { Page } from "@playwright/test";

/**
 * Wait for the Tauri app to be fully loaded.
 */
export async function waitForAppReady(page: Page): Promise<void> {
	await page.waitForSelector("#app", { state: "attached", timeout: 10_000 });
	await page
		.waitForFunction(() => !document.querySelector('[data-loading="true"]'), {
			timeout: 5_000,
		})
		.catch(() => {
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
			const { invoke } = await import("@tauri-apps/api/core");
			const result = await invoke<{ project: { id: number } }>(
				"project_register",
				{
					args: { path: p, display_name: n },
				},
			);
			return result.project.id;
		},
		[path, name] as const,
	);
}

/**
 * Spawn a session via the Tauri invoke bridge.
 * Returns the session id.
 */
export async function spawnSession(
	page: Page,
	projectId: number,
	label: string,
	opts?: { command?: string[]; workingDirectory?: string },
): Promise<number> {
	return page.evaluate(
		async ([pid, lbl, cmd, cwd]) => {
			const { invoke } = await import("@tauri-apps/api/core");
			const result = await invoke<{ session: { id: number } }>(
				"session_spawn",
				{
					args: {
						project_id: pid,
						label: lbl,
						command: cmd ?? ["/bin/sh", "-c", "sleep 3600"],
						working_directory: cwd ?? "/tmp",
					},
				},
			);
			return result.session.id;
		},
		[
			projectId,
			label,
			opts?.command ?? null,
			opts?.workingDirectory ?? null,
		] as const,
	);
}

/**
 * End a session by sending TERM signal.
 */
export async function endSession(page: Page, sessionId: number): Promise<void> {
	await page.evaluate(async (sid) => {
		const { invoke } = await import("@tauri-apps/api/core");
		await invoke("session_end", { args: { session_id: sid } });
	}, sessionId);
}

/**
 * Activate a session (brings it to foreground, ensures companion).
 */
export async function activateSession(
	page: Page,
	sessionId: number,
): Promise<void> {
	await page.evaluate(async (sid) => {
		const { invoke } = await import("@tauri-apps/api/core");
		await invoke("session_activate", { args: { session_id: sid } });
	}, sessionId);
}

/**
 * Click a session row by label to activate it via UI.
 */
export async function clickSessionRow(
	page: Page,
	label: string,
): Promise<void> {
	await page.click(`.session-row:has(.session-label:has-text("${label}"))`);
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
		state: "visible",
		timeout,
	});
}

/**
 * Acknowledge an alert on a session via the Tauri command.
 */
export async function acknowledgeAlert(
	page: Page,
	alertId: number,
): Promise<void> {
	await page.evaluate(async (aid) => {
		const { invoke } = await import("@tauri-apps/api/core");
		await invoke("session_acknowledge_alert", { args: { alert_id: aid } });
	}, alertId);
}

/**
 * Get the list of sessions from the backend.
 */
export async function getSessionList(
	page: Page,
	opts?: { projectId?: number; includeEnded?: boolean },
): Promise<Array<{ id: number; status: string; alert: unknown }>> {
	return page.evaluate(
		async ([pid, incEnded]) => {
			const { invoke } = await import("@tauri-apps/api/core");
			const result = await invoke<{
				sessions: Array<{ id: number; status: string; alert: unknown }>;
			}>("session_list", {
				args: { project_id: pid ?? null, include_ended: incEnded ?? false },
			});
			return result.sessions;
		},
		[opts?.projectId ?? null, opts?.includeEnded ?? false] as const,
	);
}
