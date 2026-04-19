// Classify a session by the CLI agent it runs, for the filter-chip UI.
//
// The backend doesn't store agent kind directly — we infer it from
// `metadata.command` (the argv the session was spawned with) and the
// session label. Matching is case-insensitive and returns the first
// matching kind in priority order; `other` is the fallback.

import type { SessionSummary } from "$lib/api/sessions";

export type AgentKind = "claude" | "copilot" | "gemini" | "codex" | "other";

export interface AgentKindMeta {
	id: AgentKind;
	label: string;
	/** Accent colour for the filter-chip dot + active state. */
	color: string;
}

export const AGENT_KIND_META: readonly AgentKindMeta[] = [
	{ id: "claude", label: "Claude", color: "#f97316" },
	{ id: "copilot", label: "Copilot", color: "#60a5fa" },
	{ id: "gemini", label: "Gemini", color: "#a855f7" },
	{ id: "codex", label: "Codex", color: "#10b981" },
];

/** Classifier — order matters: more specific matches first. */
export function sessionAgentKind(session: SessionSummary): AgentKind {
	const argv = Array.isArray(session.metadata?.command)
		? (session.metadata.command as string[])
		: [];
	const haystack = [...argv, session.label].join(" ").toLowerCase();

	if (/\bclaude(-code)?\b/.test(haystack)) return "claude";
	// `copilot` can appear via `gh copilot` or `copilot-cli`.
	if (/\b(gh\s+copilot|copilot(-cli)?)\b/.test(haystack)) return "copilot";
	if (/\bgemini\b/.test(haystack)) return "gemini";
	if (/\bcodex\b/.test(haystack)) return "codex";
	return "other";
}
