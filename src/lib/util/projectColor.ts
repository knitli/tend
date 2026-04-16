// Phase 2 review fix: single source of truth for resolving a project's
// display colour. `project.settings.color` is freeform JSON stored in SQLite
// and interpolated into `style="--project-color: {value}"` on multiple
// components. Values are validated at this boundary so callers can treat
// `null` as "fall through to the CSS var() fallback" without worrying about
// CSS injection from a malformed DB row.

import { isValidHexColor, type Project } from "$lib/api/projects";

/**
 * 16-colour palette for auto-assignment. Designed for dark backgrounds:
 * visually distinct, colourblind-friendly, and avoids clashing with the
 * UI accent blue (#60a5fa). The project's `id` (stable integer PK) is
 * used as an index so the assignment is deterministic across sessions.
 */
const AUTO_PALETTE: readonly string[] = [
	"#f472b6", // pink
	"#fb923c", // orange
	"#a78bfa", // violet
	"#34d399", // emerald
	"#fbbf24", // amber
	"#38bdf8", // sky
	"#f87171", // red
	"#4ade80", // green
	"#c084fc", // purple
	"#facc15", // yellow
	"#2dd4bf", // teal
	"#fb7185", // rose
	"#818cf8", // indigo
	"#a3e635", // lime
	"#22d3ee", // cyan
	"#e879f9", // fuchsia
];

/**
 * Return the project's display colour as a validated `#rrggbb` hex string.
 *
 * Resolution order:
 *  1. `project.settings.color` — user-chosen via the colour picker.
 *  2. Deterministic palette colour derived from `project.id`.
 *  3. `null` if the project itself is null/undefined (callers fall through
 *     to the CSS `var(--project-color, var(--color-accent, ...))` chain).
 */
export function getProjectColor(
	project: Project | null | undefined,
): string | null {
	if (!project) return null;
	const raw = project.settings?.color;
	if (isValidHexColor(raw)) return raw;
	// Auto-assign from palette using the stable project id.
	return AUTO_PALETTE[project.id % AUTO_PALETTE.length];
}
