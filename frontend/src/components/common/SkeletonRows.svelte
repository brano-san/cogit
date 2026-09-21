<script lang="ts">
  import { GRAPH } from "$lib/graph-geometry";

  interface Props {
    rows?: number;
    height?: number;
  }

  let { rows = 8, height = GRAPH.rowHeight }: Props = $props();

  /** Uneven widths: a column of identical bars reads as a rendering fault, not as loading. */
  const WIDTHS = [72, 54, 83, 61, 91, 48, 77, 66];
</script>

<div class="skeleton" aria-hidden="true">
  {#each { length: rows } as _, index (index)}
    <div class="row" style:height="{height}px">
      <span class="bar" style:width="{WIDTHS[index % WIDTHS.length]}%"></span>
    </div>
  {/each}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    padding: 0 var(--sp-5);
  }

  .bar {
    height: 8px;
    border-radius: var(--r-sm);
    background: var(--surface-raised);
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.45;
    }
    50% {
      opacity: 0.9;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bar {
      animation: none;
    }
  }
</style>
