import { describe, expect, it } from "vitest";
import type { SessionSummary } from "$lib/api/sessions";
import { sessionAgentKind } from "./agentKind";

function makeSession(command: string[] = [], label = ""): SessionSummary {
	return {
		id: 1,
		project_id: 1,
		label,
		pid: 1234,
		status: "idle",
		status_source: "ipc",
		ownership: "workbench",
		started_at: "2026-04-20T00:00:00Z",
		ended_at: null,
		last_activity_at: "2026-04-20T00:00:00Z",
		last_heartbeat_at: null,
		exit_code: null,
		error_reason: null,
		working_directory: "/tmp",
		metadata: { command },
		activity_summary: null,
		alert: null,
		reattached_mirror: false,
	};
}

describe("sessionAgentKind", () => {
	it("matches claude", () => {
		expect(sessionAgentKind(makeSession(["claude"]))).toBe("claude");
	});

	it("matches claude-code as claude", () => {
		expect(sessionAgentKind(makeSession(["claude-code"]))).toBe("claude");
	});

	it("matches gh copilot", () => {
		expect(sessionAgentKind(makeSession(["gh", "copilot"]))).toBe("copilot");
	});

	it("matches copilot-cli", () => {
		expect(sessionAgentKind(makeSession(["copilot-cli"]))).toBe("copilot");
	});

	it("applies priority ordering when multiple agents match", () => {
		expect(sessionAgentKind(makeSession(["gh", "copilot", "claude-code"]))).toBe(
			"claude",
		);
		expect(sessionAgentKind(makeSession(["gemini", "codex"]))).toBe("gemini");
	});

	it("falls back to other when nothing matches", () => {
		expect(sessionAgentKind(makeSession(["custom-tool"], "plain session"))).toBe(
			"other",
		);
	});
});
