<!-- T064: Single session row in the session list.
     Displays project name, label, status badge, ownership indicator,
     and dispatches a click event for session activation. -->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import type { SessionSummary } from '$lib/api/sessions';

  interface Props {
    session: SessionSummary;
    projectName?: string;
    onActivate?: (session: SessionSummary) => void;
  }

  let { session, projectName = '', onActivate }: Props = $props();

  let tick = $state(Date.now());
  const tickInterval = setInterval(() => { tick = Date.now(); }, 30_000);
  onDestroy(() => clearInterval(tickInterval));

  const isInteractive = $derived(
    session.ownership === 'workbench' && !session.reattached_mirror,
  );

  const statusLabel = $derived.by(() => {
    switch (session.status) {
      case 'working':
        return 'Working';
      case 'idle':
        return 'Idle';
      case 'needs_input':
        return 'Needs Input';
      case 'ended':
        return 'Ended';
      case 'error':
        return 'Error';
      default:
        return session.status;
    }
  });

  const statusClass = $derived(`status-${session.status.replace('_', '-')}`);

  const relativeTime = $derived.by(() => {
    const now = tick;
    const then = new Date(session.last_activity_at).getTime();
    if (Number.isNaN(then)) return 'unknown';
    const diffMs = now - then;
    const diffSec = Math.floor(diffMs / 1000);

    if (diffSec < 60) return 'just now';
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `${diffMin}m ago`;
    const diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return `${diffHr}h ago`;
    const diffDays = Math.floor(diffHr / 24);
    return `${diffDays}d ago`;
  });

  function handleClick(): void {
    onActivate?.(session);
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleClick();
    }
  }
</script>

<div
  class="session-row"
  role="button"
  tabindex="0"
  aria-label="{session.label} - {statusLabel}"
  onclick={handleClick}
  onkeydown={handleKeydown}
>
  <div class="session-main">
    <div class="session-label-row">
      <span class="session-label">{session.label}</span>
      {#if !isInteractive}
        <span class="badge badge-readonly" title="Read-only session">RO</span>
      {/if}
    </div>
    {#if projectName}
      <span class="session-project">{projectName}</span>
    {/if}
  </div>

  <div class="session-meta">
    <span class="badge {statusClass}">{statusLabel}</span>
    {#if session.alert}
      <span class="badge badge-alert badge-pulse" title={session.alert.reason ?? 'Needs input'} aria-label="Needs input">!</span>
    {/if}
    <span class="session-time">{relativeTime}</span>
  </div>
</div>

<style>
  .session-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3, 0.75rem);
    padding: var(--space-2, 0.5rem) var(--space-4, 1rem);
    cursor: pointer;
    transition: background 150ms;
    outline: none;
    border-bottom: 1px solid var(--color-border-subtle, #1e2028);
  }

  .session-row:hover,
  .session-row:focus-visible {
    background: var(--color-surface-hover, #1e2028);
  }

  .session-main {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .session-label-row {
    display: flex;
    align-items: center;
    gap: var(--space-2, 0.5rem);
  }

  .session-label {
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--color-text, #e6e8ef);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-project {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-meta {
    display: flex;
    align-items: center;
    gap: var(--space-2, 0.5rem);
    flex-shrink: 0;
  }

  .session-time {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.6875rem;
    font-weight: 500;
    line-height: 1.4;
    white-space: nowrap;
  }

  .status-working {
    background: var(--color-success-bg, #064e3b);
    color: var(--color-success, #34d399);
  }

  .status-idle {
    background: var(--color-info-bg, #1e3a5f);
    color: var(--color-info, #60a5fa);
  }

  .status-needs-input {
    background: var(--color-warning-bg, #713f12);
    color: var(--color-warning, #fbbf24);
  }

  .status-ended {
    background: var(--color-muted-bg, #1e2028);
    color: var(--color-text-muted, #8b8fa3);
  }

  .status-error {
    background: var(--color-error-bg, #4c0519);
    color: var(--color-error, #f87171);
  }

  .badge-alert {
    background: var(--color-warning-bg, #713f12);
    color: var(--color-warning, #fbbf24);
    font-weight: 700;
  }

  .badge-readonly {
    background: var(--color-muted-bg, #1e2028);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.625rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .badge-pulse {
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
