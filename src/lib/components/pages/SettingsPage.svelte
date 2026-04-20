<!--
  Full-page Settings view. Replaces the SettingsDialog modal. Sections are
  visually modelled on the reference mock (Data Source / Development
  Directories / Data Sync / Telemetry / About) but adapted to tend's
  actual backend surface — notifications, project-parent directories and
  a sync refresh that re-hydrates the session/project stores.
-->
<script lang="ts">
  import PageHeader from '$lib/components/PageHeader.svelte';
  import SpinnerIcon from '$lib/components/SpinnerIcon.svelte';
  import {
    notificationPreferenceGet,
    notificationPreferenceSet,
    type NotificationChannel,
    type QuietHours,
  } from '$lib/api/notifications';
  import packageJson from '../../../../package.json';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionsStore } from '$lib/stores/sessions.svelte';

  // ── Notifications (global preference) ──────────────────────────────
  let channels = $state<NotificationChannel[]>(['in_app', 'os_notification']);
  let quietEnabled = $state(false);
  let quietStart = $state('22:00');
  let quietEnd = $state('07:00');
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  let savedResetTimer: ReturnType<typeof setTimeout> | null = null;

  function clearSavedResetTimer(): void {
    if (savedResetTimer) {
      clearTimeout(savedResetTimer);
      savedResetTimer = null;
    }
  }

  $effect(() => {
    // Fire-and-forget load on mount.
    loadPreference();
  });

  $effect(() => {
    return () => {
      clearSavedResetTimer();
    };
  });

  async function loadPreference(): Promise<void> {
    loadError = null;
    try {
      const { preference } = await notificationPreferenceGet();
      channels = [...preference.channels];
      if (preference.quiet_hours) {
        quietEnabled = true;
        quietStart = preference.quiet_hours.start;
        quietEnd = preference.quiet_hours.end;
      } else {
        quietEnabled = false;
      }
    } catch (err) {
      loadError = err instanceof Error ? err.message : String(err);
    }
  }

  function toggleChannel(ch: NotificationChannel): void {
    channels = channels.includes(ch)
      ? channels.filter((c) => c !== ch)
      : [...channels, ch];
  }

  async function saveNotifications(): Promise<void> {
    saving = true;
    saved = false;
    try {
      const quiet: QuietHours | undefined = quietEnabled
        ? { start: quietStart, end: quietEnd, timezone: 'local' }
        : undefined;
      await notificationPreferenceSet({ channels, quietHours: quiet });
      saved = true;
      clearSavedResetTimer();
      savedResetTimer = setTimeout(() => {
        saved = false;
        savedResetTimer = null;
      }, 1500);
    } catch (err) {
      loadError = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }

  // ── Data Sync ──────────────────────────────────────────────────────
  let syncing = $state(false);
  let syncError = $state<string | null>(null);

  function getSyncErrorMessage(err: unknown): string {
    return (
      projectsStore.error ||
      sessionsStore.error ||
      (err instanceof Error ? err.message : String(err))
    );
  }

  async function resync(): Promise<void> {
    syncing = true;
    syncError = null;
    try {
      await Promise.all([
        projectsStore.hydrate({ includeArchived: true }),
        sessionsStore.hydrate({ includeEnded: false }),
      ]);
      const storeError = projectsStore.error || sessionsStore.error;
      if (storeError) syncError = storeError;
    } catch (err) {
      syncError = getSyncErrorMessage(err);
    } finally {
      syncing = false;
    }
  }

  // ── Derived: unique parent directories of registered projects ──────
  const projectParents = $derived.by<string[]>(() => {
    const seen = new Set<string>();
    for (const p of projectsStore.activeProjects) {
      const canonicalPath = p.canonical_path.replace(/[\\/]+$/, '');
      const idx = Math.max(
        canonicalPath.lastIndexOf('/'),
        canonicalPath.lastIndexOf('\\'),
      );
      if (idx > 0) seen.add(canonicalPath.slice(0, idx));
    }
    return Array.from(seen).sort();
  });

  const appVersion = packageJson.version;
</script>

<div class="settings-page">
  <PageHeader title="Settings" subtitle="Configure tend" />

  <div class="settings-body">
    <!-- Data Source — informational; tend persists sessions via its own
         workbench DB managed by the Rust backend. Locations are read-only. -->
    <section class="card">
      <h2 class="card-title">
        <span class="icon" aria-hidden="true">🗂</span> Data Source
      </h2>
      <div class="field">
        <span class="field-label">Workbench database</span>
        <code class="path">Managed by tend in your local app data directory</code>
      </div>
      <div class="field">
        <span class="field-label">Daemon socket</span>
        <code class="path">Managed by tend in its local runtime/app data location</code>
      </div>
      <p class="field-hint">
        tend stores all state locally. Exact locations are platform-dependent
        and managed by the workbench — read-only.
      </p>
    </section>

    <!-- Development Directories — derived from registered projects. We don't
         enforce a restrict-list today; this is informational. -->
    <section class="card">
      <h2 class="card-title">
        <span class="icon" aria-hidden="true">📁</span> Development Directories
      </h2>
      <p class="card-subtitle">
        Parent directories of the projects you've registered with tend.
      </p>
      {#if projectParents.length === 0}
        <p class="empty">No projects registered yet.</p>
      {:else}
        <ul class="dir-list">
          {#each projectParents as dir (dir)}
            <li><code class="path">{dir}</code></li>
          {/each}
        </ul>
      {/if}
    </section>

    <!-- Notifications (existing backend) -->
    <section class="card">
      <h2 class="card-title">
        <span class="icon" aria-hidden="true">🔔</span> Notifications
      </h2>
      <p class="card-subtitle">Configure default notification preferences for tend.</p>

      {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}

      <fieldset class="inner">
        <legend>Channels</legend>
        <label class="check">
          <input type="checkbox" checked={channels.includes('in_app')} onchange={() => toggleChannel('in_app')} />
          In-app alert bar
        </label>
        <label class="check">
          <input type="checkbox" checked={channels.includes('os_notification')} onchange={() => toggleChannel('os_notification')} />
          OS notification
        </label>
        <label class="check">
          <input type="checkbox" checked={channels.includes('terminal_bell')} onchange={() => toggleChannel('terminal_bell')} />
          Terminal bell
        </label>
        <label class="check">
          <input type="checkbox" checked={channels.includes('silent')} onchange={() => toggleChannel('silent')} />
          Silent (suppress all)
        </label>
      </fieldset>

      <fieldset class="inner">
        <legend>Quiet hours</legend>
        <label class="check">
          <input type="checkbox" bind:checked={quietEnabled} />
          Enable quiet hours
        </label>
        {#if quietEnabled}
          <div class="time-row">
            <label>Start<input type="time" bind:value={quietStart} /></label>
            <label>End<input type="time" bind:value={quietEnd} /></label>
          </div>
          <p class="field-hint">
            OS notifications are suppressed during quiet hours. In-app alerts still appear.
          </p>
        {/if}
      </fieldset>

      <div class="actions">
        <button class="btn primary" onclick={saveNotifications} disabled={saving}>
          {saving ? 'Saving…' : saved ? 'Saved ✓' : 'Save'}
        </button>
      </div>
    </section>

    <!-- Data Sync -->
    <section class="card">
      <h2 class="card-title">
        <span class="icon" aria-hidden="true">⟳</span> Data Sync
      </h2>
      <p class="card-subtitle">Re-hydrate projects and sessions from the workbench backend.</p>
      {#if syncError}<p class="error" role="alert">{syncError}</p>{/if}
      <div class="actions">
        <button class="btn primary" onclick={resync} disabled={syncing}>
          {#if syncing}<SpinnerIcon />Syncing…{:else}Re-sync Now{/if}
        </button>
      </div>
    </section>

    <!-- About -->
    <section class="card">
      <h2 class="card-title">
        <span class="icon" aria-hidden="true">ⓘ</span> About
      </h2>
      <div class="field">
        <span class="field-label">Version</span>
        <span>{appVersion}</span>
      </div>
      <div class="field">
        <span class="field-label">Data access</span>
        <span>Local only — all state stays on this machine.</span>
      </div>
      <p class="field-hint">
        tend is a local agent orchestration workbench. Session data, scratchpads,
        and workspace layouts never leave your machine.
      </p>
    </section>
  </div>
</div>

<style>
  .settings-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: var(--color-surface, #0f1115);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem 1.5rem 2rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    max-width: 720px;
    width: 100%;
  }

  .card {
    background: var(--color-surface-raised, #15171c);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.5rem;
    padding: 1rem 1.125rem;
  }

  .card-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0 0 0.5rem;
    font-size: 0.9375rem;
    font-weight: 600;
    color: var(--color-text, #e6e8ef);
  }

  .icon {
    font-size: 1rem;
  }

  .card-subtitle {
    margin: 0 0 0.75rem;
    font-size: 0.8125rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .field {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    margin: 0.25rem 0;
    font-size: 0.8125rem;
  }

  .field-label {
    color: var(--color-text-muted, #8b8fa3);
    min-width: 140px;
  }

  .field-hint {
    margin: 0.5rem 0 0;
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .path {
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.75rem;
    padding: 0.125rem 0.375rem;
    background: var(--color-surface, #0f1115);
    border-radius: 0.25rem;
    color: var(--color-text, #e6e8ef);
  }

  .dir-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .empty {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  fieldset.inner {
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    padding: 0.625rem 0.75rem;
    margin: 0 0 0.75rem;
  }

  fieldset.inner legend {
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0 0.25rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.1875rem 0;
    font-size: 0.8125rem;
    color: var(--color-text, #e6e8ef);
    cursor: pointer;
  }

  .time-row {
    display: flex;
    gap: 1rem;
    margin: 0.5rem 0;
  }

  .time-row label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.75rem;
    color: var(--color-text-muted, #8b8fa3);
  }

  .time-row input[type='time'] {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.25rem;
    background: var(--color-surface, #0f1115);
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }

  .btn {
    padding: 0.375rem 0.875rem;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    background: transparent;
    color: var(--color-text, #e6e8ef);
    font-size: 0.8125rem;
    font-family: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
  }

  .btn:hover:not(:disabled) {
    background: var(--color-surface-hover, #1e2028);
  }

  .btn.primary {
    background: var(--color-accent, #60a5fa);
    color: var(--color-surface, #0f1115);
    border-color: var(--color-accent, #60a5fa);
    font-weight: 500;
  }

  .btn.primary:hover:not(:disabled) {
    background: var(--color-accent-hover, #93c5fd);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .error {
    margin: 0 0 0.5rem;
    padding: 0.5rem;
    background: rgba(248, 113, 113, 0.1);
    border: 1px solid var(--color-error, #f87171);
    border-radius: 0.25rem;
    color: var(--color-error, #f87171);
    font-size: 0.8125rem;
  }
</style>
