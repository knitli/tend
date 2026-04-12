// T062: Svelte 5 runes store for session state.
// Manages a reactive Map of sessions, grouped-by-project derived view,
// and auto-subscribes to backend events for real-time updates.

import {
  sessionList,
  onSessionSpawned,
  onSessionEnded,
  onSessionOutput,
  type SessionSummary,
  type SessionStatus,
} from '$lib/api/sessions';
import type { UnlistenFn } from '@tauri-apps/api/event';

export interface SessionOutputHandler {
  (sessionId: number, bytes: string): void;
}

function createSessionsStore() {
  let sessionsMap = $state<Map<number, SessionSummary>>(new Map());
  let loading = $state(false);
  let error = $state<string | null>(null);
  let outputHandler = $state<SessionOutputHandler | null>(null);

  const sessions = $derived(Array.from(sessionsMap.values()));

  const activeSessions = $derived(
    sessions.filter((s) => s.status !== 'ended' && s.status !== 'error'),
  );

  /** Sessions grouped by project_id, sorted by last_activity_at descending. */
  const byProject = $derived.by(() => {
    const groups = new Map<number, SessionSummary[]>();
    for (const session of sessions) {
      const list = groups.get(session.project_id);
      if (list) {
        list.push(session);
      } else {
        groups.set(session.project_id, [session]);
      }
    }
    // Sort each group by last_activity_at descending
    for (const list of groups.values()) {
      list.sort((a, b) =>
        b.last_activity_at.localeCompare(a.last_activity_at),
      );
    }
    return groups;
  });

  /** Sessions that have an active alert. */
  const sessionsWithAlerts = $derived(
    sessions.filter((s) => s.alert !== null),
  );

  // Event unlisten handles for cleanup
  let unlisteners: UnlistenFn[] = [];

  return {
    get sessions() {
      return sessions;
    },
    get activeSessions() {
      return activeSessions;
    },
    get byProject() {
      return byProject;
    },
    get sessionsWithAlerts() {
      return sessionsWithAlerts;
    },
    get loading() {
      return loading;
    },
    get error() {
      return error;
    },

    /** Fetch all sessions from the backend and replace local state. */
    async hydrate(opts?: {
      projectId?: number;
      includeEnded?: boolean;
    }): Promise<void> {
      loading = true;
      error = null;
      try {
        const result = await sessionList(opts);
        const next = new Map<number, SessionSummary>();
        for (const s of result.sessions) {
          next.set(s.id, s);
        }
        sessionsMap = next;
      } catch (err) {
        error = err instanceof Error ? err.message : String(err);
      } finally {
        loading = false;
      }
    },

    /** Add or replace a session in the store. */
    add(session: SessionSummary): void {
      const next = new Map(sessionsMap);
      next.set(session.id, session);
      sessionsMap = next;
    },

    /** Patch fields on an existing session. */
    update(id: number, patch: Partial<SessionSummary>): void {
      const existing = sessionsMap.get(id);
      if (!existing) return;
      const next = new Map(sessionsMap);
      next.set(id, { ...existing, ...patch });
      sessionsMap = next;
    },

    /** Remove a session from the store. */
    remove(id: number): void {
      const next = new Map(sessionsMap);
      next.delete(id);
      sessionsMap = next;
    },

    /** Find a session by id. */
    byId(id: number): SessionSummary | undefined {
      return sessionsMap.get(id);
    },

    /** Get sessions for a specific project. */
    forProject(projectId: number): SessionSummary[] {
      return byProject.get(projectId) ?? [];
    },

    /**
     * Register a handler for session output bytes.
     * The handler receives the session id and base64-encoded bytes.
     */
    setOutputHandler(handler: SessionOutputHandler): void {
      outputHandler = handler;
    },

    /**
     * Subscribe to backend session events. Call once on app mount.
     * Returns a cleanup function that unsubscribes all listeners.
     */
    async subscribe(): Promise<() => void> {
      const spawned = await onSessionSpawned((payload) => {
        // The spawned event contains a SessionSummaryLite; we need to
        // promote it to a full SessionSummary with default derived fields.
        const lite = payload.session;
        const summary: SessionSummary = {
          id: lite.id,
          project_id: lite.project_id,
          label: lite.label,
          pid: null,
          status: lite.status,
          status_source: 'ipc',
          ownership: lite.ownership,
          started_at: new Date().toISOString(),
          ended_at: null,
          last_activity_at: new Date().toISOString(),
          metadata: {},
          working_directory: '',
          activity_summary: null,
          alert: null,
          reattached_mirror: lite.reattached_mirror,
        };
        this.add(summary);
      });

      const ended = await onSessionEnded((payload) => {
        const status: SessionStatus = 'ended';
        this.update(payload.session_id, {
          status,
          ended_at: new Date().toISOString(),
          pid: null,
        });
      });

      const output = await onSessionOutput((payload) => {
        // Update last_activity_at and forward bytes to the output handler
        this.update(payload.session_id, {
          last_activity_at: new Date().toISOString(),
        });
        if (outputHandler) {
          outputHandler(payload.session_id, payload.bytes);
        }
      });

      unlisteners = [spawned, ended, output];

      return () => {
        for (const unlisten of unlisteners) {
          unlisten();
        }
        unlisteners = [];
      };
    },
  };
}

export const sessionsStore = createSessionsStore();
