<script lang="ts">
  /** The scrollbar of a view that moves its code sideways itself (lib/code-scroll.ts):
      a native bar over a spacer as far past the bar as the text can move. */
  interface Props {
    offset: number;
    /** How far the text can move; nothing is drawn at zero. */
    max: number;
    onscroll: (offset: number) => void;
  }

  let { offset, max, onscroll }: Props = $props();

  let bar: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (bar && Math.abs(bar.scrollLeft - offset) >= 1) bar.scrollLeft = offset;
  });
</script>

{#if max > 0}
  <div class="bar" bind:this={bar} onscroll={() => bar && onscroll(bar.scrollLeft)}>
    <div class="spacer" style:width="calc(100% + {max}px)"></div>
  </div>
{/if}

<style>
  /* As tall as the app's scrollbars (app.css), so the bar is the whole element. */
  .bar {
    flex: 0 0 auto;
    height: 10px;
    overflow-x: auto;
    overflow-y: hidden;
  }

  .spacer {
    height: 1px;
  }
</style>
