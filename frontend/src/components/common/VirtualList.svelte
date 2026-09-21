<script lang="ts" generics="T">
  import { GRAPH, visibleRange } from "$lib/graph-geometry";
  import type { Snippet } from "svelte";

  interface Props {
    items: readonly T[];
    /** Absolutely positioned at `index * rowHeight`; the caller draws, this places. */
    row: Snippet<[T, number]>;
    rowHeight?: number;
    /** Rendered beyond the viewport so a fast scroll does not show blanks. */
    buffer?: number;
    label?: string;
    /** Scrolled to on change, if it is off screen. Null leaves the scroll alone. */
    reveal?: number | null;
    class?: string;
  }

  let {
    items,
    row,
    rowHeight = GRAPH.rowHeight,
    buffer = 10,
    label,
    reveal = null,
    class: extra = "",
  }: Props = $props();

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const range = $derived(
    visibleRange(scrollTop, viewportHeight, rowHeight, items.length, buffer),
  );
  const visible = $derived(
    items.slice(range.start, range.end).map((item, index) => ({ item, at: range.start + index })),
  );

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) viewportHeight = entry.contentRect.height;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  $effect(() => {
    if (reveal === null || !scroller) return;
    const top = reveal * rowHeight;
    if (top < scrollTop) scroller.scrollTop = top;
    else if (top + rowHeight > scrollTop + viewportHeight) {
      scroller.scrollTop = top + rowHeight - viewportHeight;
    }
  });
</script>

<div
  class="scroll {extra}"
  bind:this={scroller}
  onscroll={() => scroller && (scrollTop = scroller.scrollTop)}
  role={label ? "list" : undefined}
  aria-label={label}
>
  <div class="rows" style:height="{items.length * rowHeight}px">
    {#each visible as entry (entry.at)}
      {@render row(entry.item, entry.at)}
    {/each}
  </div>
</div>

<style>
  .scroll {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .rows {
    position: relative;
  }
</style>
