<!--
  Left-hand navigation rail. Five sections (Workspaces / Dashboard / Sessions /
  Projects / Settings) rendered as labelled icon rows. Retractable via the
  same edge-toggle pattern as the project Sidebar — when collapsed the rail
  disappears and the toggle stays pinned to the left edge.

  The rail owns no state; the parent drives `value` (persisted via
  `workspace.ui.active_view`) and `open` (persisted via
  `workspace.ui.nav_rail_collapsed`).
-->
<script lang="ts">
  import { Collapsible } from 'bits-ui';

  export type NavId = 'workspaces' | 'dashboard' | 'sessions' | 'projects' | 'settings';

  interface Props {
    value: NavId;
    onChange: (next: NavId) => void;
    open?: boolean;
    onToggle?: (nextOpen: boolean) => void;
    contentId?: string;
  }

  let {
    value,
    onChange,
    open = true,
    onToggle,
    contentId = 'nav-rail-content',
  }: Props = $props();

  interface NavItem {
    id: NavId;
    label: string;
    /** Inline SVG path data (24x24 viewBox). Lucide-inspired glyphs kept inline
     *  to avoid pulling in an icon package for five glyphs. */
    iconPath: string;
  }

  const items: readonly NavItem[] = [
    {
      id: 'workspaces',
      label: 'Workspaces',
      iconPath:
        'M12 2 2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5',
    },
    {
      id: 'dashboard',
      label: 'Dashboard',
      iconPath: 'M3 3h7v9H3zM14 3h7v5h-7zM14 12h7v9h-7zM3 16h7v5H3z',
    },
    {
      id: 'sessions',
      label: 'Sessions',
      iconPath: 'M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z',
    },
    {
      id: 'projects',
      label: 'Projects',
      iconPath:
        'M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z',
    },
    {
      id: 'settings',
      label: 'Settings',
      iconPath:
        'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z',
    },
  ];

  function handleKeydown(event: KeyboardEvent, id: NavId): void {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onChange(id);
    }
  }
</script>

<Collapsible.Root {open}>
  <div class="nav-rail-wrapper" class:collapsed={!open}>
    <Collapsible.Content id={contentId} forceMount class="nav-rail-collapsible">
      <nav
        class="nav-rail"
        aria-label="Primary"
        aria-hidden={!open}
        inert={!open}
      >
        <ul class="nav-list" role="list">
          {#each items as item (item.id)}
            <li>
              <button
                type="button"
                class="nav-item"
                class:active={value === item.id}
                aria-current={value === item.id ? 'page' : undefined}
                onclick={() => onChange(item.id)}
                onkeydown={(e) => handleKeydown(e, item.id)}
              >
                <svg
                  class="nav-icon"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <path d={item.iconPath} />
                </svg>
                <span class="nav-label">{item.label}</span>
              </button>
            </li>
          {/each}
        </ul>
      </nav>
    </Collapsible.Content>

    <button
      type="button"
      class="nav-edge-toggle"
      aria-controls={contentId}
      aria-expanded={open}
      aria-label={open ? 'Collapse navigation' : 'Expand navigation'}
      title={open ? 'Collapse navigation' : 'Expand navigation'}
      onclick={() => onToggle?.(!open)}
    >
      <span class="edge-grip" aria-hidden="true">
        <span class="grip-dots"></span>
      </span>
      <span class="edge-chevron" aria-hidden="true">{open ? '‹' : '›'}</span>
    </button>
  </div>
</Collapsible.Root>

<style>
  .nav-rail-wrapper {
    display: flex;
    flex-shrink: 0;
    height: 100%;
    position: relative;
  }

  :global(.nav-rail-collapsible) {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex-shrink: 0;
    overflow: hidden;
    transition: width 200ms ease;
  }

  :global(.nav-rail-collapsible[data-state='open']) {
    width: 200px;
  }

  :global(.nav-rail-collapsible[data-state='closed']) {
    width: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.nav-rail-collapsible) {
      transition: none;
    }
  }

  .nav-rail {
    width: 200px;
    height: 100%;
    background: var(--color-surface-raised, #15171c);
    padding: 1rem 0.5rem;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .nav-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    width: 100%;
    padding: 0.5rem 0.625rem;
    border: none;
    border-radius: 0.375rem;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.8125rem;
    font-family: inherit;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: background 150ms, color 150ms;
  }

  .nav-item:hover {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .nav-item:focus-visible {
    outline: 2px solid var(--color-accent, #60a5fa);
    outline-offset: 2px;
  }

  .nav-item.active {
    background: var(--color-accent-soft, rgba(96, 165, 250, 0.18));
    color: var(--color-accent, #60a5fa);
  }

  .nav-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .nav-label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Edge toggle — mirrors Sidebar.svelte so behaviour matches. */
  .nav-edge-toggle {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 12px;
    height: 100%;
    padding: 0;
    border: none;
    border-left: 1px solid var(--color-border, #2a2d35);
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    flex-shrink: 0;
    position: relative;
    transition: background 150ms, width 150ms;
  }

  .nav-edge-toggle:hover {
    width: 16px;
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }

  .nav-edge-toggle:focus-visible {
    outline: 2px solid var(--color-accent, #60a5fa);
    outline-offset: -2px;
  }

  .edge-grip {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .grip-dots {
    display: block;
    width: 4px;
    height: 32px;
    background-image: radial-gradient(circle, currentColor 1px, transparent 1px);
    background-size: 4px 4px;
    background-position: 0 0;
    opacity: 0.4;
    transition: opacity 150ms;
  }

  .nav-edge-toggle:hover .grip-dots {
    opacity: 0.8;
  }

  .edge-chevron {
    font-size: 0.625rem;
    line-height: 1;
    padding-bottom: 4px;
    opacity: 0;
    transition: opacity 150ms;
  }

  .nav-edge-toggle:hover .edge-chevron {
    opacity: 1;
  }

  .collapsed .nav-edge-toggle {
    width: 16px;
    border-left: none;
    border-right: 1px solid var(--color-border, #2a2d35);
  }

  .collapsed .nav-edge-toggle:hover {
    width: 20px;
  }

  .collapsed .edge-chevron {
    opacity: 0.6;
  }
</style>
