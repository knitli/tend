/**
 * Shared E2E test helpers for the Tauri app.
 *
 * Helpers call the injected mock (`window.__TAURI_INTERNALS__.invoke`) directly
 * instead of dynamically importing `@tauri-apps/api/core` — bare specifiers do
 * not resolve inside `page.evaluate`, which is a raw browser context with no
 * Vite module graph.
 *
 * The `args: { snake_case }` envelope mirrors the real Tauri command shape
 * (`#[tauri::command] fn foo(args: FooArgs)`); the mock unwraps it the same
 * way the real backend would.
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
 * Low-level invoke that goes through the injected `__TAURI_INTERNALS__`.
 * Use this for ad-hoc commands that don't have a dedicated helper.
 */
export async function invoke<T = unknown>(
	page: Page,
	command: string,
	args?: Record<string, unknown>,
): Promise<T> {
	return page.evaluate(
		async ([cmd, a]) => {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const internals = (window as any).__TAURI_INTERNALS__;
			if (!internals) throw new Error("mock-tauri not injected");
			return internals.invoke(cmd, a) as Promise<unknown>;
		},
		[command, args ?? {}] as const,
	) as Promise<T>;
}

/**
 * Reset the mock backend's in-memory state. Useful between tests in the same
 * page if you want a clean slate without a full reload.
 */
export async function resetMock(page: Page): Promise<void> {
	await page.evaluate(() => {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		(window as any).__MOCK_TAURI__?.reset();
	});
}

/**
 * Register a project via the mocked invoke bridge.
 * Returns the project id.
 */
export async function registerProject(
	page: Page,
	path: string,
	name: string,
): Promise<number> {
	const result = await invoke<{ project: { id: number } }>(
		page,
		"project_register",
		{ args: { path, display_name: name } },
	);
	return result.project.id;
}

/**
 * Spawn a session via the mocked invoke bridge.
 */
export async function spawnSession(
	page: Page,
	projectId: number,
	label: string,
	opts?: { command?: string[]; workingDirectory?: string },
): Promise<number> {
	const result = await invoke<{ session: { id: number } }>(
		page,
		"session_spawn",
		{
			args: {
				project_id: projectId,
				label,
				command: opts?.command ?? ["/bin/sh", "-c", "sleep 3600"],
				working_directory: opts?.workingDirectory ?? "/tmp",
			},
		},
	);
	return result.session.id;
}

/**
 * End a session.
 */
export async function endSession(page: Page, sessionId: number): Promise<void> {
	await invoke(page, "session_end", { args: { session_id: sessionId } });
}

/**
 * Activate a session (creates companion if needed).
 */
export async function activateSession(
	page: Page,
	sessionId: number,
): Promise<void> {
	await invoke(page, "session_activate", { args: { session_id: sessionId } });
}

/**
 * Click a session row by label.
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
 * Acknowledge an alert.
 */
export async function acknowledgeAlert(
	page: Page,
	alertId: number,
): Promise<void> {
	await invoke(page, "session_acknowledge_alert", {
		args: { alert_id: alertId },
	});
}

/**
 * Get the list of sessions from the (mocked) backend.
 */
export async function getSessionList(
	page: Page,
	opts?: { projectId?: number; includeEnded?: boolean },
): Promise<Array<{ id: number; status: string; alert: unknown }>> {
	const result = await invoke<{
		sessions: Array<{ id: number; status: string; alert: unknown }>;
	}>(page, "session_list", {
		args: {
			project_id: opts?.projectId ?? null,
			include_ended: opts?.includeEnded ?? false,
		},
	});
	return result.sessions;
}

/**
 * Simulate a backend-side alert raise (which in production comes through the
 * daemon IPC needs_input path, not a Tauri command).
 */
export async function raiseMockAlert(
	page: Page,
	sessionId: number,
	reason?: string,
): Promise<{ id: number }> {
	return page.evaluate(
		([sid, r]) => {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			return (window as any).__MOCK_TAURI__.raiseAlert(sid, r ?? undefined);
		},
		[sessionId, reason ?? null] as const,
	);
}
