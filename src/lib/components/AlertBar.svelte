<!--
  T082: AlertBar — pinned above the session list, showing all open alerts
  with project + session label + reason + "acknowledge" button.
-->
<script lang="ts">
  import { sessionsStore } from '$lib/stores/sessions.svelte';
  import { projectsStore } from '$lib/stores/projects.svelte';
  import { sessionAcknowledgeAlert } from '$lib/api/notifications';
  import type { SessionSummary } from '$lib/api/sessions';

  interface Props {
    onActivateSession?: (session: SessionSummary) => void;
  }

  let { onActivateSession }: Props = $props();

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

  /** P1-B: activate the session AND emit a scroll-to event so the session list
   *  scrolls the row into view. The event is handled by SessionList. */
  function goToSession(session: SessionSummary): void {
    onActivateSession?.(session);
    window.dispatchEvent(
      new CustomEvent('tend:session-scroll-to', {
        detail: { sessionId: session.id },
      }),
    );
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
              onclick={() => goToSession(session)}
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
    background: var(--color-warning-bg, #3d2e00);
    border-bottom: 1px solid var(--color-warning, #fbbf24);
    padding: 0.5rem 1rem;
    font-size: 0.875rem;
    color: var(--color-text, #e6e8ef);
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
    background: var(--color-warning, #fbbf24);
    color: var(--color-surface, #0f1115);
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
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
  }

  .alert-separator {
    color: var(--color-text-muted, #8b8fa3);
    opacity: 0.5;
  }

  .alert-label {
    font-weight: 500;
    color: var(--color-text, #e6e8ef);
  }

  .alert-reason {
    color: var(--color-text-muted, #8b8fa3);
    font-style: italic;
  }

  .alert-go-btn {
    margin-left: auto;
    padding: 0.125rem 0.5rem;
    border: 1px solid var(--color-accent, #60a5fa);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-accent, #60a5fa);
    cursor: pointer;
    font-size: 0.75rem;
    transition: background-color 0.15s;
  }

  .alert-go-btn:hover {
    background: color-mix(in srgb, var(--color-accent, #60a5fa) 15%, transparent);
  }

  .alert-ack-btn {
    padding: 0.125rem 0.5rem;
    border: 1px solid var(--color-warning, #fbbf24);
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-warning, #fbbf24);
    cursor: pointer;
    font-size: 0.75rem;
    transition: background-color 0.15s;
  }

  .alert-ack-btn:hover {
    background: color-mix(in srgb, var(--color-warning, #fbbf24) 15%, transparent);
  }
</style>
