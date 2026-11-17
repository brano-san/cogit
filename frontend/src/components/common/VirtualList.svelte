<script lang="ts" generics="T">
  import { visibleRange } from "$lib/graph-geometry";
  import { anchoredTop, visibleRows, type RowGeometry } from "$lib/diff-wrap";
  import { wheelGlide } from "$lib/wheel-glide";
  import { uiMetricsStore } from "$stores/ui-metrics.svelte";
  import { untrack, type Snippet } from "svelte";

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
    geometry?: RowGeometry | null;
    ondrawn?: () => void;
  }

  let {
    items,
    row,
    rowHeight: fixedRowHeight,
    buffer = 10,
    label,
    reveal = null,
    geometry = null,
    ondrawn,
  }: Props = $props();
  /** A caller's own height, else the list row of the UI font. */
  const rowHeight = $derived(fixedRowHeight ?? uiMetricsStore.listRow);

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  /** One wheel speed however little the list overflows (lib/wheel-glide.ts). */
  const glide = wheelGlide(
    () => scroller,
    () => rowHeight,
  );
  $effect(() => glide.stop);
  let viewportHeight = $state(0);

  const range = $derived(
    geometry
      ? visibleRows(geometry, scrollTop, viewportHeight, buffer)
      : visibleRange(scrollTop, viewportHeight, rowHeight, items.length, buffer),
  );
  // Numbers, not the range object: a scroll inside one row changes neither, so the rows are
  // not looked at again.
  const start = $derived(range.start);
  const end = $derived(range.end);
  /** The indices drawn. A row is keyed and valued by its index, so one that stays on screen
      is not drawn again when the window moves: a one-row scroll draws only the row that came
      (wrapping each item in a new object per scroll redrew every row). */
  const shown = $derived(Array.from({ length: end - start }, (_, k) => start + k));
  const total = $derived(geometry ? geometry.total : items.length * rowHeight);

  $effect(() => {
    void shown;
    void geometry;
    if (ondrawn) untrack(ondrawn);
  });

  let placed: RowGeometry | null = null;
  $effect(() => {
    const now = geometry;
    untrack(() => {
      const before = placed;
      placed = now;
      if (!before || !now || !scroller || before.count !== now.count) return;
      const top = anchoredTop(before, now, scroller.scrollTop);
      if (top !== scroller.scrollTop) scroller.scrollTop = top;
    });
  });

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
    glide.stop();
    const at = untrack(() => geometry);
    const top = at ? at.top(reveal) : reveal * rowHeight;
    const tall = at ? at.height(reveal) : rowHeight;
    const shown = scroller.scrollTop;
    const height = scroller.clientHeight;
    if (top < shown || (at !== null && tall > height)) scroller.scrollTop = top;
    else if (top + tall > shown + height) {
      scroller.scrollTop = top + tall - height;
    }
  });
</script>

<div
  class="scroll key-list"
  bind:this={scroller}
  onscroll={() => scroller && (scrollTop = scroller.scrollTop)}
  onwheel={glide.onwheel}
  onpointerdown={glide.stop}
  onkeydown={glide.stop}
  role={label ? "list" : undefined}
  aria-label={label}
>
  <div class="rows" style:height="{total}px">
    {#each shown as at (at)}
      <!-- A derived per row: new items with the same object at this index draw nothing. -->
      {@const item = items[at] as T}
      {@render row(item, at)}
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
