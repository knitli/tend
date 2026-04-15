<!--
  P1-D: SpinnerIcon — small CSS-only rotating arc used as an inline loading
  indicator on async buttons (Refresh, Layouts dropdown, etc). No SVG file
  dependency; uses a single div with border + border-top-color and a
  @keyframes rotation. Respects `prefers-reduced-motion` by falling back to a
  static filled dot.
-->
<script lang="ts">
  interface Props {
    /** Diameter in pixels. Default 14. */
    size?: number;
    /** Accessible label. Defaults to "Loading". */
    label?: string;
  }

  let { size = 14, label = 'Loading' }: Props = $props();
</script>

<span
  class="spinner-icon"
  role="status"
  aria-label={label}
  style="--spinner-size: {size}px"
></span>

<style>
  .spinner-icon {
    display: inline-block;
    width: var(--spinner-size, 14px);
    height: var(--spinner-size, 14px);
    border-radius: 50%;
    border: 2px solid var(--color-border, #2a2d35);
    border-top-color: currentColor;
    animation: spinner-rotate 1s linear infinite;
    vertical-align: middle;
    box-sizing: border-box;
  }

  @keyframes spinner-rotate {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner-icon {
      /* Static filled dot fallback — no motion, still communicates "busy". */
      animation: none;
      border: none;
      background: currentColor;
    }
  }
</style>
