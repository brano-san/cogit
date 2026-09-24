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
  }

  let {
    items,
    row,
    rowHeight = GRAPH.rowHeight,
    buffer = 10,
    label,
    reveal = null,
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

  // Only a change of `reveal` scrolls. The position is read from the element, not from
  // the state the scroll handler writes: depending on that, every wheel turn ran this
  // again and pulled the view back to the row.
  $effect(() => {
    if (reveal === null || !scroller) return;
    const top = reveal * rowHeight;
    const shown = scroller.scrollTop;
    const height = scroller.clientHeight;
    if (top < shown) scroller.scrollTop = top;
    else if (top + rowHeight > shown + height) {
      scroller.scrollTop = top + rowHeight - height;
    }
  });
</script>

<div
  class="scroll"
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
