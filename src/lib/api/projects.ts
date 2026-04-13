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
	readonly [key: string]: unknown;
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
