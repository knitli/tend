// T080: typed wrappers for notification Tauri commands + event subscribers.

import { invoke } from './invoke';
import {
  listen,
  type AlertRaisedEvent,
  type AlertClearedEvent,
} from './events';
import type { UnlistenFn } from '@tauri-apps/api/event';

// ---------- Types ----------

export type NotificationChannel = 'in_app' | 'os_notification' | 'terminal_bell' | 'silent';

export interface QuietHours {
  readonly start: string; // "HH:MM"
  readonly end: string;   // "HH:MM"
  readonly timezone: string; // "local"
}

export interface NotificationPreference {
  readonly id: number;
  readonly project_id: number | null;
  readonly channels: NotificationChannel[];
  readonly quiet_hours: QuietHours | null;
  readonly updated_at: string;
}

// ---------- Commands ----------

/**
 * Get notification preferences (project-specific or global).
 */
export async function notificationPreferenceGet(
  opts?: { projectId?: number },
): Promise<{ preference: NotificationPreference }> {
  return invoke<{ preference: NotificationPreference }>('notification_preference_get', {
    args: { project_id: opts?.projectId ?? null },
  });
}

/**
 * Set notification preferences (project-specific or global).
 */
export async function notificationPreferenceSet(
  opts: {
    projectId?: number;
    channels: NotificationChannel[];
    quietHours?: QuietHours;
  },
): Promise<{ preference: NotificationPreference }> {
  return invoke<{ preference: NotificationPreference }>('notification_preference_set', {
    args: {
      project_id: opts.projectId ?? null,
      channels: opts.channels,
      quiet_hours: opts.quietHours ?? null,
    },
  });
}

/**
 * Acknowledge (clear) a needs_input alert.
 */
export async function sessionAcknowledgeAlert(
  opts: { sessionId: number; alertId: number },
): Promise<void> {
  await invoke<Record<string, never>>('session_acknowledge_alert', {
    args: { session_id: opts.sessionId, alert_id: opts.alertId },
  });
}

// ---------- Event subscribers ----------

/**
 * Subscribe to alert:raised events.
 */
export function onAlertRaised(
  cb: (payload: AlertRaisedEvent) => void,
): Promise<UnlistenFn> {
  return listen('alert:raised', cb);
}

/**
 * Subscribe to alert:cleared events.
 */
export function onAlertCleared(
  cb: (payload: AlertClearedEvent) => void,
): Promise<UnlistenFn> {
  return listen('alert:cleared', cb);
}
