<!-- T064: Single session row in the session list.
     Displays project name, label, status badge, ownership indicator,
     and dispatches a click event for session activation. -->
<script lang="ts" module>
  // M11 fix: Single shared ticker for all SessionRow instances.
  // Avoids N independent intervals, ensures consistent display.
  let sharedTick = $state(Date.now());
  let refCount = 0;
  let sharedInterval: ReturnType<typeof setInterval> | null = null;

  function acquireTick(): void {
    refCount++;
    if (refCount === 1) {
      sharedInterval = setInterval(() => { sharedTick = Date.now(); }, 5_000);
    }
  }

  function releaseTick(): void {
    refCount--;
    if (refCount <= 0 && sharedInterval !== null) {
      clearInterval(sharedInterval);
      sharedInterval = null;
      refCount = 0;
    }
  }
</script>

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { SessionSummary } from '$lib/api/sessions';

  interface Props {
    session: SessionSummary;
    projectName?: string;
    /** True when this session was referenced by a layout restore but is no longer running. */
    missing?: boolean;
    /** True when this session is currently visible in a pane slot (Phase 1: the activeSessionId). */
    active?: boolean;
    /** True when any session in the list is active — used to dim non-active rows. */
    anyActive?: boolean;
    /** Phase 2-C: project colour hex (e.g. `#60a5fa`). When absent the row
     *  uses the global `--color-accent` fallback via CSS `var()`. */
    projectColor?: string | null;
    onActivate?: (session: SessionSummary) => void;
    /** P4-D: keyboard-accessible "add to pane" button. When provided, a
     *  small `⊞` button is rendered next to the status badge; click
     *  invokes this callback with the session. This is the non-drag
     *  equivalent of dropping the row onto the AddSlotZone. */
    onOpenInSlot?: (session: SessionSummary) => void;
  }

  let {
    session,
    projectName = '',
    missing = false,
    active = false,
    anyActive = false,
    projectColor = null,
    onActivate,
    onOpenInSlot,
  }: Props = $props();

  const tick = $derived(sharedTick);
  onMount(() => acquireTick());
  onDestroy(() => releaseTick());

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

  const statusClass = $derived(`status-${session.status.replaceAll('_', '-')}`);

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

  /** T137: Idle time display for idle sessions. */
  const idleTime = $derived.by(() => {
    if (session.status !== 'idle') return null;
    const now = tick;
    const then = new Date(session.last_activity_at).getTime();
    if (Number.isNaN(then)) return null;
    const diffSec = Math.floor((now - then) / 1000);
    if (diffSec < 60) return `idle ${diffSec}s`;
    const diffMin = Math.floor(diffSec / 60);
    if (diffMin < 60) return `idle ${diffMin}m`;
    const diffHr = Math.floor(diffMin / 60);
    return `idle ${diffHr}h`;
  });

  /** T137: Activity summary truncated to ~60 chars for the row display.
   *  M7 fix: use Array.from for surrogate-safe truncation. */
  const displaySummary = $derived.by(() => {
    const raw = session.activity_summary;
    if (!raw) return null;
    const chars = Array.from(raw);
    if (chars.length <= 60) return raw;
    return chars.slice(0, 59).join('') + '…';
  });

  /** T137: Task title from agent metadata, shown as a pill.
   *  M9 fix: cap at 200 chars to prevent unbounded agent metadata. */
  const taskTitle = $derived.by(() => {
    const raw = session.metadata?.task_title;
    if (!raw) return null;
    if (raw.length <= 200) return raw;
    return raw.slice(0, 199) + '…';
  });

  /** H7: Comprehensive aria-label including alert and read-only state. */
  const ariaLabel = $derived(
    [
      projectName ? `${projectName}:` : '',
      session.label,
      '-',
      statusLabel,
      session.alert ? `(alert: ${session.alert.reason ?? 'needs input'})` : '',
      !isInteractive ? '(read-only)' : '',
    ]
      .filter(Boolean)
      .join(' '),
  );

  function handleClick(): void {
    onActivate?.(session);
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleClick();
    }
  }

  /** P4-D: the ⊞ button uses stopPropagation so clicking it doesn't also
   *  fire the row's onActivate (which would replace the active slot
   *  instead of appending a new one). */
  function handleOpenInSlot(event: MouseEvent): void {
    event.stopPropagation();
    onOpenInSlot?.(session);
  }
</script>

<div
  class="session-row"
  class:active
  class:dimmed={!active && anyActive}
  data-session-id={session.id}
  style={projectColor ? `--project-color: ${projectColor}` : ''}
  role="button"
  tabindex="0"
  aria-label={ariaLabel}
  onclick={handleClick}
  onkeydown={handleKeydown}
>
  <div class="session-main">
    <div class="session-label-row">
      <span class="session-label">{session.label}</span>
      {#if missing}
        <span class="badge badge-missing" title="Session no longer running">not running</span>
      {/if}
      {#if !isInteractive}
        <span class="badge badge-readonly" title="Read-only session">RO</span>
      {/if}
    </div>
    {#if taskTitle}
      <span class="task-pill" title={taskTitle}>{taskTitle}</span>
    {/if}
    {#if projectName}
      <span class="session-project">{projectName}</span>
    {/if}
    {#if displaySummary}
      <span class="activity-summary" title={session.activity_summary ?? ''}>{displaySummary}</span>
    {:else if idleTime}
      <span class="activity-summary idle-time" role="status" aria-live="polite">{idleTime}</span>
    {/if}
  </div>

  <div class="session-meta">
    <span class="badge {statusClass}">{statusLabel}</span>
    {#if session.alert}
      <span class="badge badge-alert badge-pulse" role="img" title={session.alert.reason ?? 'Needs input'} aria-label={session.alert.reason ?? 'Needs input'}>!</span>
    {/if}
    {#if onOpenInSlot}
      <button
        type="button"
        class="open-in-slot-btn"
        onclick={handleOpenInSlot}
        onkeydown={(e) => e.stopPropagation()}
        title="Open in pane"
        aria-label="Open {session.label} in a new pane"
      >
        ⊞
      </button>
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
    border-left: 2px solid transparent;
  }

  .session-row:hover,
  .session-row:focus-visible {
    background: var(--color-surface-hover, #1e2028);
  }

  .session-row:focus-visible {
    outline: 2px solid var(--color-accent, #60a5fa);
    outline-offset: -2px;
  }

  /* P1-A: Active session indicator. --project-color falls back to --color-accent
     when not set (Phase 2 will populate --project-color per project). */
  .session-row.active {
    border-left-color: var(--project-color, var(--color-accent, #60a5fa));
    background: color-mix(
      in srgb,
      var(--project-color, var(--color-accent, #60a5fa)) 8%,
      var(--color-surface, #0f1115)
    );
  }

  /* Spec §8.1: dot sits on the row root (inside the outer gutter), not
     nested inside the label flex row. Placing it on `.session-row::before`
     makes it the first flex child, so the existing row `gap` separates it
     from the `.session-main` column. */
  .session-row.active::before {
    content: '';
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--project-color, var(--color-accent, #60a5fa));
    flex-shrink: 0;
  }

  /* Dim non-active rows' main text when any session is active. Badges stay
     fully opaque because they live in .session-meta, not .session-main. */
  .session-row.dimmed .session-main {
    opacity: 0.7;
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

  /* P4-D: keyboard-accessible "add to pane" button. Visible only when
     onOpenInSlot prop is supplied (i.e. when the row lives inside the
     pane-workspace-capable page). Click handler uses stopPropagation so
     activating the row's onActivate isn't fired alongside. */
  .open-in-slot-btn {
    width: 20px;
    height: 20px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
    line-height: 1;
    cursor: pointer;
    transition: background 120ms, color 120ms, border-color 120ms;
    flex-shrink: 0;
  }

  .open-in-slot-btn:hover,
  .open-in-slot-btn:focus-visible {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
    border-color: var(--color-accent, #60a5fa);
    outline: none;
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
    background: var(--color-warning-bg, #3d2e00);
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
    background: var(--color-warning-bg, #3d2e00);
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

  .activity-summary {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    opacity: 0.85;
  }

  .idle-time {
    font-style: italic;
    opacity: 0.65;
  }

  .task-pill {
    display: inline-flex;
    align-items: center;
    padding: 0 5px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.625rem;
    font-weight: 500;
    line-height: 1.4;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
    background: var(--color-accent-bg, rgba(96, 165, 250, 0.12));
    color: var(--color-accent, #60a5fa);
  }

  .badge-missing {
    background: var(--color-muted-bg, #1e2028);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.5625rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    opacity: 0.8;
  }

  .badge-pulse {
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  @media (prefers-reduced-motion: reduce) {
    .badge-pulse {
      animation: none;
    }
  }
</style>
