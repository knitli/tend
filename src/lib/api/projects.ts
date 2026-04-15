// T059: typed wrappers for project management Tauri commands.
// Mirrors the project surface from contracts/tauri-commands.md §1.

import { invoke } from "./invoke";

// ---------- Types ----------

export interface Project {
	readonly id: number;
	readonly canonical_path: string;
	readonly display_name: string;
	readonly added_at: string;
	readonly last_active_at: string | null;
	readonly archived_at: string | null;
	readonly settings: ProjectSettings;
}

export interface ProjectSettings {
	readonly retention_days?: number;
	/**
	 * Spec §1 adaptive-ui: per-project display colour (hex string, e.g. `#60a5fa`).
	 * Auto-assigned from a 12-colour palette on `project_register` when absent.
	 * May be overridden by the user via the Sidebar colour picker.
	 */
	readonly color?: string;
	readonly [key: string]: unknown;
}

// ---------- Validators ----------

/**
 * Narrow a freeform DB/JSON value to a validated `#rrggbb` hex colour.
 *
 * `project.settings.color` is freeform JSON loaded straight from SQLite and
 * later interpolated into `style="--project-color: {value}"` inline styles.
 * CSS custom-property values are NOT subject to Svelte's attribute-text
 * escaping, so a malformed value (hand-edited DB, future import flow, bug)
 * could produce CSS injection. `vanilla-colorful` is safe at write time, but
 * consumers should always re-validate at the DOM boundary.
 *
 * Accepts: `#60a5fa`, `#FFF000`. Rejects everything else (named colours,
 * 3-digit shorthand, surrounding whitespace, trailing garbage, non-strings).
 */
export function isValidHexColor(value: unknown): value is string {
	return typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value);
}

// ---------- Commands ----------

/**
 * List registered projects.
 * When `includeArchived` is true, soft-deleted projects are included.
 */
export async function projectList(opts?: {
	includeArchived?: boolean;
}): Promise<{ projects: Project[] }> {
	return invoke<{ projects: Project[] }>("project_list", {
		args: { include_archived: opts?.includeArchived ?? false },
	});
}

/**
 * Register a new project directory.
 * The backend canonicalizes the path and creates the project row.
 */
export async function projectRegister(opts: {
	path: string;
	displayName?: string;
}): Promise<{ project: Project }> {
	return invoke<{ project: Project }>("project_register", {
		args: { path: opts.path, display_name: opts.displayName },
	});
}

/**
 * Update a project's display name or settings.
 */
export async function projectUpdate(opts: {
	id: number;
	displayName?: string;
	settings?: ProjectSettings;
}): Promise<{ project: Project }> {
	return invoke<{ project: Project }>("project_update", {
		args: {
			id: opts.id,
			display_name: opts.displayName,
			settings: opts.settings,
		},
	});
}

/**
 * Soft-delete a project. Ends all live sessions and stops watchers.
 */
export async function projectArchive(opts: { id: number }): Promise<void> {
	await invoke<Record<string, never>>("project_archive", {
		args: { id: opts.id },
	});
}

/**
 * Restore a previously archived project.
 */
export async function projectUnarchive(opts: {
	id: number;
}): Promise<{ project: Project }> {
	return invoke<{ project: Project }>("project_unarchive", {
		args: { id: opts.id },
	});
}
