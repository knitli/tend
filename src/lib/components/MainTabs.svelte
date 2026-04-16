<!-- P4-F: Top-level tabs dividing the main panel into Sessions / Workspace /
     Overview views. This is a thin wrapper around bits-ui's `Tabs.*`
     primitives that accepts snippet props for each tab's body, because
     Svelte 5 doesn't support React-style slot distribution and the parent
     `+page.svelte` needs to decide what renders inside each tab.

     The `value` prop is a narrowed `TabId` union, but bits-ui's Tabs.Root
     expects a plain `string` — we bridge that at the boundary by casting
     on the way in (value={value}) and the way out (in `onValueChange`).
     This keeps the public API type-safe while satisfying the library's
     string-typed primitive.
 -->
<script lang="ts">
  import { Tabs } from 'bits-ui';
  import type { Snippet } from 'svelte';

  export type TabId = 'sessions' | 'workspace' | 'overview';

  interface Props {
    value: TabId;
    onValueChange: (v: TabId) => void;
    sessionsContent: Snippet;
    workspaceContent: Snippet;
    overviewContent: Snippet;
  }

  let {
    value,
    onValueChange,
    sessionsContent,
    workspaceContent,
    overviewContent,
  }: Props = $props();
</script>

<Tabs.Root
  class="main-tabs-root"
  value={value}
  onValueChange={(v) => onValueChange(v as TabId)}
>
  <Tabs.List class="main-tabs-list" aria-label="Main view">
    <Tabs.Trigger class="main-tabs-trigger" value="sessions">Sessions</Tabs.Trigger>
    <Tabs.Trigger class="main-tabs-trigger" value="workspace">Workspace</Tabs.Trigger>
    <Tabs.Trigger class="main-tabs-trigger" value="overview">Overview</Tabs.Trigger>
  </Tabs.List>

  <!-- bits-ui mounts all three Tabs.Content regions and toggles visibility
       via the `hidden` attribute. For the tend workbench that would be
       disastrous — each tab's body owns a SessionList (with a `bind:this`)
       and a PaneWorkspace (which mounts xterm instances tied to PTY
       forwarding). We side-step this by only INVOKING the snippet for the
       currently-active tab; the inactive Tabs.Content regions still exist
       in the DOM (empty) so bits-ui's ARIA + keyboard navigation continue
       to work as expected. -->
  <Tabs.Content class="main-tabs-content" value="sessions">
    {#if value === 'sessions'}{@render sessionsContent()}{/if}
  </Tabs.Content>
  <Tabs.Content class="main-tabs-content" value="workspace">
    {#if value === 'workspace'}{@render workspaceContent()}{/if}
  </Tabs.Content>
  <Tabs.Content class="main-tabs-content" value="overview">
    {#if value === 'overview'}{@render overviewContent()}{/if}
  </Tabs.Content>
</Tabs.Root>

<style>
  /* The Tabs.Root div needs to be a flex column filling its parent so that
     the Tabs.Content region can expand to fill the remaining vertical space
     below the tab strip. */
  :global(.main-tabs-root) {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  :global(.main-tabs-list) {
    display: flex;
    gap: 0;
    padding: 0 0.75rem;
    border-bottom: 1px solid var(--color-border, #2a2d35);
    background: var(--color-surface, #0f1115);
    flex-shrink: 0;
  }

  :global(.main-tabs-trigger) {
    padding: 0.5rem 1rem;
    border: none;
    background: transparent;
    color: var(--color-text-muted, #8b8fa3);
    cursor: pointer;
    font-size: 0.8125rem;
    font-family: inherit;
    font-weight: 500;
    line-height: 1.3;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px; /* overlap the list border so the active underline
                            replaces it cleanly */
    transition: color 120ms, border-color 120ms, background 120ms;
  }

  :global(.main-tabs-trigger:hover) {
    color: var(--color-text, #e6e8ef);
    background: var(--color-surface-hover, #1a1d25);
  }

  :global(.main-tabs-trigger:focus-visible) {
    outline: 2px solid var(--color-accent, #60a5fa);
    outline-offset: -2px;
  }

  :global(.main-tabs-trigger[data-state="active"]) {
    color: var(--color-text, #e6e8ef);
    border-bottom-color: var(--color-accent, #60a5fa);
  }

  /* Each Tabs.Content region owns the remaining vertical space when active.
     bits-ui applies `hidden` to inactive content, so a single rule works.
     `align-items: stretch` ensures children fill the cross-axis (width)
     and `justify-content: flex-start` keeps content top-aligned instead of
     centering or end-aligning when the child doesn't fill all vertical space. */
  :global(.main-tabs-content) {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    justify-content: flex-start;
    overflow: hidden;
  }

  /* bits-ui toggles the `hidden` attribute on inactive Tabs.Content divs.
     Some browsers set `hidden` elements to `display: none !important` which
     overrides our `display: flex`. Override it so the layout rule persists
     and only `hidden` / aria-hidden controls visibility. */
  :global(.main-tabs-content[hidden]) {
    display: none !important;
  }
</style>
