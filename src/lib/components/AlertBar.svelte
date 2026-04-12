<!--
  T082: AlertBar — pinned above the session list, showing all open alerts
  with project + session label + reason + "acknowledge" button.
-->
<script lang="ts">
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionAcknowledgeAlert } from '$lib/api/notifications';
  import type { SessionSummary } from '$lib/api/sessions';

  const { onActivateSession } = $props<{
    onActivateSession?: (session: SessionSummary) => void;
  }>();

  const alerts = $derived(sessionsStore.sessionsWithAlerts);

  function projectName(projectId: number): string {
    const project = projectsStore.byId(projectId);
    return project?.display_name ?? `Project #${projectId}`;
  }

  async function acknowledge(sessionId: number, alertId: number) {
    try {
      await sessionAcknowledgeAlert({ sessionId, alertId });
    } catch (err) {
      // Silently ignore — the event bus will update the store when the
      // backend confirms the clear.
    }
  }
</script>

{#if alerts.length > 0}
  <div class="alert-bar" role="status" aria-live="polite">
    <div class="alert-bar-header">
      <span class="alert-icon" aria-hidden="true">!</span>
      <span class="alert-count">{alerts.length} session{alerts.length === 1 ? '' : 's'} need{alerts.length === 1 ? 's' : ''} input</span>
    </div>
    <ul class="alert-list">
      {#each alerts as session (session.id)}
        {@const alert = session.alert}
        {#if alert}
          <li class="alert-item">
            <span class="alert-project">{projectName(session.project_id)}</span>
            <span class="alert-separator">/</span>
            <span class="alert-label">{session.label}</span>
            {#if alert.reason}
              <span class="alert-reason">— {alert.reason}</span>
            {/if}
            <button
              class="alert-go-btn"
              onclick={() => onActivateSession?.(session)}
              title="Jump to session"
            >
              Go to
            </button>
            <button
              class="alert-ack-btn"
              onclick={() => acknowledge(session.id, alert.id)}
              title="Acknowledge alert"
            >
              Dismiss
            </button>
          </li>
        {/if}
      {/each}
    </ul>
  </div>
{/if}

<style>
  .alert-bar {
    background: var(--color-warning-bg, #fef3c7);
    border-bottom: 1px solid var(--color-warning-border, #f59e0b);
    padding: 0.5rem 1rem;
    font-size: 0.875rem;
  }

  .alert-bar-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    margin-bottom: 0.25rem;
  }

  .alert-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    background: var(--color-warning, #f59e0b);
    color: white;
    border-radius: 50%;
    font-size: 0.75rem;
    font-weight: 700;
  }

  .alert-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .alert-item {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.25rem 0;
  }

  .alert-project {
    color: var(--color-text-muted, #6b7280);
    font-size: 0.8125rem;
  }

  .alert-separator {
    color: var(--color-text-muted, #6b7280);
    opacity: 0.5;
  }

  .alert-label {
    font-weight: 500;
  }

  .alert-reason {
    color: var(--color-text-muted, #6b7280);
    font-style: italic;
  }

  .alert-go-btn {
    margin-left: auto;
    padding: 0.125rem 0.5rem;
    border: 1px solid var(--color-info-border, #3b82f6);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-info-text, #1d4ed8);
    cursor: pointer;
    font-size: 0.75rem;
    transition: background-color 0.15s;
  }

  .alert-go-btn:hover {
    background: var(--color-info-hover, #dbeafe);
  }

  .alert-ack-btn {
    padding: 0.125rem 0.5rem;
    border: 1px solid var(--color-warning-border, #f59e0b);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-warning-text, #92400e);
    cursor: pointer;
    font-size: 0.75rem;
    transition: background-color 0.15s;
  }

  .alert-ack-btn:hover {
    background: var(--color-warning-hover, #fde68a);
  }
</style>
