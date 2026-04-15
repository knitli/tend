<!--
  Phase 2-B: Project colour picker popover.

  Wraps the `<hex-color-picker>` custom element from `vanilla-colorful`
  (2.7 KB, zero-deps, framework-agnostic) in a small popover anchored
  to the invoking colour swatch button. Registers the custom element
  lazily on mount so SSR is not impacted.

  Dismiss paths:
    - Escape key
    - Click outside the popover
    - Explicit `onClose` call from the parent

  Colour changes fire on every drag tick as the `<hex-color-picker>`
  emits its `color-changed` event — the parent is responsible for
  debouncing the resulting `projectUpdate` if it cares about write
  amplification. For a handful of projects this is cheap enough to
  fire-and-forget.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';

  interface Props {
    /** Current hex colour (e.g. `#60a5fa`). */
    value: string;
    /** Called on every colour change (on drag or click). */
    onChange: (hex: string) => void;
    /** Called when the popover should close (Escape, click-outside). */
    onClose: () => void;
  }

  let { value, onChange, onClose }: Props = $props();

  let rootEl: HTMLDivElement | undefined = $state();
  let pickerEl: HTMLElement | undefined = $state();
  /** True once `vanilla-colorful/hex-color-picker.js` has been imported and
   *  the `<hex-color-picker>` custom element is registered. Before this is
   *  true, rendering the element would produce a plain `HTMLUnknownElement`
   *  with no UI. */
  let ready = $state(false);

  function handleColorChanged(event: Event): void {
    // vanilla-colorful's CustomEvent carries { value: "#rrggbb" }.
    const detail = (event as CustomEvent<{ value: string }>).detail;
    if (detail && typeof detail.value === 'string') {
      onChange(detail.value);
    }
  }

  function handleDocumentClick(event: MouseEvent): void {
    if (rootEl && !rootEl.contains(event.target as Node)) {
      onClose();
    }
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }

  onMount(() => {
    // SSR-safe dynamic import: only load the custom element in the browser.
    if (typeof window !== 'undefined') {
      import('vanilla-colorful/hex-color-picker.js')
        .then(() => {
          ready = true;
        })
        .catch(() => {
          // Swallow: the picker simply stays empty if the module fails to
          // load. The swatch button still shows the colour.
          ready = false;
        });
    }

    // Defer the click-outside listener to the next tick so the same click
    // that opened the popover doesn't immediately close it.
    const raf = requestAnimationFrame(() => {
      document.addEventListener('click', handleDocumentClick, true);
    });

    return () => {
      cancelAnimationFrame(raf);
    };
  });

  onDestroy(() => {
    document.removeEventListener('click', handleDocumentClick, true);
    if (pickerEl) {
      pickerEl.removeEventListener('color-changed', handleColorChanged);
    }
  });

  // Attach the `color-changed` listener once the custom element is rendered
  // and the module has been imported. Re-attaches if `pickerEl` changes.
  $effect(() => {
    if (!ready || !pickerEl) return;
    pickerEl.addEventListener('color-changed', handleColorChanged);
    return () => {
      pickerEl?.removeEventListener('color-changed', handleColorChanged);
    };
  });

  // Keep the picker's `color` attribute in sync with the current value so
  // drag-handles on the saturation/hue surfaces start from the right spot.
  $effect(() => {
    if (!ready || !pickerEl) return;
    // The element reads its `color` attribute; set via DOM to avoid a
    // Svelte warning about unknown HTML attributes on a custom element.
    (pickerEl as unknown as { color: string }).color = value;
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="color-swatch-picker"
  role="dialog"
  aria-label="Pick project colour"
  tabindex="-1"
  bind:this={rootEl}
  onkeydown={handleKeydown}
>
  {#if ready}
    <hex-color-picker bind:this={pickerEl} color={value}></hex-color-picker>
  {:else}
    <div class="color-swatch-picker-loading" aria-hidden="true">…</div>
  {/if}
  <div class="color-swatch-picker-value" aria-live="polite">{value}</div>
</div>

<style>
  .color-swatch-picker {
    position: absolute;
    top: 100%;
    right: 0;
    z-index: 100;
    margin-top: 0.25rem;
    padding: 0.5rem;
    background: var(--color-surface, #0f1115);
    border: 1px solid var(--color-border, #2a2d35);
    border-radius: 0.375rem;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  /* vanilla-colorful exposes its internal surfaces as CSS Shadow Parts.
     Minimal dark-theme fit: give the picker a compact footprint and a
     subtle border around the draggable surfaces so they read as distinct
     from the popover background. */
  .color-swatch-picker :global(hex-color-picker) {
    width: 200px;
    height: 160px;
    --vanilla-colorful-width: 200px;
    --vanilla-colorful-height: 160px;
  }

  .color-swatch-picker :global(hex-color-picker::part(saturation)),
  .color-swatch-picker :global(hex-color-picker::part(hue)) {
    border-radius: 4px;
  }

  .color-swatch-picker-loading {
    width: 200px;
    height: 160px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted, #8b8fa3);
    font-size: 0.75rem;
  }

  .color-swatch-picker-value {
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.6875rem;
    color: var(--color-text-muted, #8b8fa3);
    text-align: center;
    user-select: all;
  }
</style>
