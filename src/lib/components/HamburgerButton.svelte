<!--
  P3-A: Hamburger button for toggling the collapsible sidebar.

  Rendered as a 32x32 button fixed at the top-left of `.main-panel`. Always
  visible so the user can reopen the sidebar regardless of its state. The
  button does NOT use the bits-ui `Collapsible.Trigger` component because we
  need the button outside the Collapsible subtree (so it stays visible even
  when the Sidebar is collapsed to width: 0). Instead we forward an `onToggle`
  callback that the parent wires to the same `open` state that drives the
  Collapsible.Root `open` prop.

  Accessibility:
  - aria-label describes the action
  - aria-expanded mirrors the open state for assistive tech
  - aria-controls points to the Collapsible.Content id so AT can associate the
    trigger with the region it controls
-->
<script lang="ts">
  interface Props {
    /** Current open state of the sidebar. */
    open: boolean;
    /** Id of the Collapsible.Content element this button controls. */
    controlsId: string;
    /** Called when the user clicks the button. The parent flips the `open`
     *  state in response; this component does not own the state itself so the
     *  same toggle can also be triggered by hover-peek or keyboard. */
    onToggle: (next: boolean) => void;
  }

  let { open, controlsId, onToggle }: Props = $props();
</script>

<button
  type="button"
  class="hamburger"
  aria-label="Toggle projects sidebar"
  aria-expanded={open}
  aria-controls={controlsId}
  onclick={() => onToggle(!open)}
>
  <span aria-hidden="true">☰</span>
</button>

<style>
  .hamburger {
    /* 32 x 32 fixed hit target per design spec §7. Positioned by the parent
       (.main-panel) — this component is layout-agnostic. */
    width: 32px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: var(--radius-sm, 4px);
    background: var(--color-surface-raised, #15171c);
    color: var(--color-text-muted, #8b8fa3);
    font-size: 1rem;
    line-height: 1;
    cursor: pointer;
    transition: background 150ms, color 150ms;
  }

  .hamburger:hover,
  .hamburger:focus-visible {
    background: var(--color-surface-hover, #1e2028);
    color: var(--color-text, #e6e8ef);
  }
</style>
