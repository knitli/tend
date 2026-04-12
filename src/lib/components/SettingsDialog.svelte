<!--
  T083: SettingsDialog — notification preferences form.
  Channel select + quiet hours start/end; wires to notification_preference_get/set.
-->
<script lang="ts">
  import {
    notificationPreferenceGet,
    notificationPreferenceSet,
    type NotificationChannel,
    type NotificationPreference,
    type QuietHours,
  } from '$lib/api/notifications';

  interface Props {
    projectId?: number;
    open: boolean;
    onclose: () => void;
  }

  let { projectId, open, onclose }: Props = $props();

  let channels = $state<NotificationChannel[]>(['in_app', 'os_notification']);
  let quietStart = $state('22:00');
  let quietEnd = $state('07:00');
  let quietEnabled = $state(false);
  let saving = $state(false);
  let loadError = $state<string | null>(null);

  $effect(() => {
    if (open) {
      loadPreference();
    }
  });

  async function loadPreference() {
    loadError = null;
    try {
      const result = await notificationPreferenceGet({ projectId });
      channels = [...result.preference.channels];
      if (result.preference.quiet_hours) {
        quietEnabled = true;
        quietStart = result.preference.quiet_hours.start;
        quietEnd = result.preference.quiet_hours.end;
      } else {
        quietEnabled = false;
      }
    } catch (err) {
      loadError = err instanceof Error ? err.message : String(err);
    }
  }

  function toggleChannel(ch: NotificationChannel) {
    if (channels.includes(ch)) {
      channels = channels.filter((c) => c !== ch);
    } else {
      channels = [...channels, ch];
    }
  }

  async function save() {
    saving = true;
    try {
      const quietHours: QuietHours | undefined = quietEnabled
        ? { start: quietStart, end: quietEnd, timezone: 'local' }
        : undefined;
      await notificationPreferenceSet({
        projectId,
        channels,
        quietHours,
      });
      onclose();
    } catch (err) {
      loadError = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <div class="dialog-overlay" role="presentation" onclick={onclose}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="dialog"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Notification settings"
      onclick={(e) => e.stopPropagation()}
    >
      <h2 class="dialog-title">Notification Settings</h2>
      {#if projectId}
        <p class="dialog-subtitle">Project-specific preferences</p>
      {:else}
        <p class="dialog-subtitle">Global default preferences</p>
      {/if}

      {#if loadError}
        <p class="dialog-error">{loadError}</p>
      {/if}

      <fieldset class="channel-fieldset">
        <legend>Notification channels</legend>
        <label class="channel-option">
          <input
            type="checkbox"
            checked={channels.includes('in_app')}
            onchange={() => toggleChannel('in_app')}
          />
          In-app alert bar
        </label>
        <label class="channel-option">
          <input
            type="checkbox"
            checked={channels.includes('os_notification')}
            onchange={() => toggleChannel('os_notification')}
          />
          OS notification
        </label>
        <label class="channel-option">
          <input
            type="checkbox"
            checked={channels.includes('terminal_bell')}
            onchange={() => toggleChannel('terminal_bell')}
          />
          Terminal bell
        </label>
        <label class="channel-option">
          <input
            type="checkbox"
            checked={channels.includes('silent')}
            onchange={() => toggleChannel('silent')}
          />
          Silent (suppress all)
        </label>
      </fieldset>

      <fieldset class="quiet-fieldset">
        <legend>Quiet hours</legend>
        <label class="quiet-toggle">
          <input type="checkbox" bind:checked={quietEnabled} />
          Enable quiet hours
        </label>
        {#if quietEnabled}
          <div class="quiet-times">
            <label>
              Start
              <input type="time" bind:value={quietStart} />
            </label>
            <label>
              End
              <input type="time" bind:value={quietEnd} />
            </label>
          </div>
          <p class="quiet-note">
            During quiet hours, OS notifications are suppressed. In-app alerts still appear.
          </p>
        {/if}
      </fieldset>

      <div class="dialog-actions">
        <button class="btn-cancel" onclick={onclose}>Cancel</button>
        <button class="btn-save" onclick={save} disabled={saving}>
          {saving ? 'Saving...' : 'Save'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .dialog {
    background: var(--color-surface, #fff);
    border-radius: 0.5rem;
    padding: 1.5rem;
    width: 100%;
    max-width: 28rem;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  }

  .dialog-title {
    margin: 0 0 0.25rem;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .dialog-subtitle {
    margin: 0 0 1rem;
    font-size: 0.8125rem;
    color: var(--color-text-muted, #6b7280);
  }

  .dialog-error {
    color: var(--color-error, #dc2626);
    font-size: 0.8125rem;
    margin: 0 0 0.75rem;
  }

  fieldset {
    border: 1px solid var(--color-border, #e5e7eb);
    border-radius: 0.375rem;
    padding: 0.75rem;
    margin: 0 0 1rem;
  }

  legend {
    font-size: 0.8125rem;
    font-weight: 500;
    padding: 0 0.25rem;
  }

  .channel-option {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .quiet-toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    cursor: pointer;
    margin-bottom: 0.5rem;
  }

  .quiet-times {
    display: flex;
    gap: 1rem;
    margin-bottom: 0.5rem;
  }

  .quiet-times label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.8125rem;
  }

  .quiet-times input[type='time'] {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--color-border, #e5e7eb);
    border-radius: 0.25rem;
    font-size: 0.875rem;
  }

  .quiet-note {
    font-size: 0.75rem;
    color: var(--color-text-muted, #6b7280);
    margin: 0;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .btn-cancel,
  .btn-save {
    padding: 0.375rem 1rem;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .btn-cancel {
    background: transparent;
    border: 1px solid var(--color-border, #e5e7eb);
  }

  .btn-save {
    background: var(--color-primary, #3b82f6);
    color: white;
    border: none;
  }

  .btn-save:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
