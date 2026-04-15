// Phase 2 review fix: single source of truth for resolving a project's
// display colour. `project.settings.color` is freeform JSON stored in SQLite
// and interpolated into `style="--project-color: {value}"` on multiple
// components. Values are validated at this boundary so callers can treat
// `null` as "fall through to the CSS var() fallback" without worrying about
// CSS injection from a malformed DB row.

import { isValidHexColor, type Project } from "$lib/api/projects";

/**
 * Return the project's display colour as a validated `#rrggbb` hex string,
 * or `null` when the project has no colour set (or an invalid value that we
 * refuse to interpolate). Callers should use the result for the inline CSS
 * custom property and let `var(--project-color, var(--color-accent, ...))`
 * handle the fallback.
 */
export function getProjectColor(
	project: Project | null | undefined,
): string | null {
	const raw = project?.settings?.color;
	return isValidHexColor(raw) ? raw : null;
}
