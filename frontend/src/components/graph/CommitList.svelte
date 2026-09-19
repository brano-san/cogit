<script lang="ts">
  import GraphCanvas from "$components/graph/GraphCanvas.svelte";
  import { formatCommitDate, shortOid } from "$lib/format";
  import { GRAPH, gutterWidth, hitTest, visibleRange } from "$lib/graph-geometry";
  import { graph } from "$stores/graph.svelte";

  /** Rows rendered beyond the viewport so a fast scroll does not show blanks. */
  const BUFFER_ROWS = 10;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let viewportWidth = $state(0);
  let selected = $state<string | null>(null);

  const total = $derived(graph.rows.length);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, total, BUFFER_ROWS),
  );
  const gutter = $derived(gutterWidth(graph.maxLane, viewportWidth || 600));

  const visible = $derived(
    graph.rows.slice(range.start, range.end).map((row, offset) => ({
      row,
      index: range.start + offset,
    })),
  );

  const nodes = $derived(
    visible.map(({ row }) => ({
      row: row.lane.row,
      lane: row.lane.lane,
      color: row.lane.color,
      merge: row.lane.kind === "merge",
      root: row.lane.kind === "root",
    })),
  );

  function onscroll() {
    if (scroller) scrollTop = scroller.scrollTop;
  }

  function onclick(event: MouseEvent) {
    if (!scroller) return;
    const box = scroller.getBoundingClientRect();
    const hit = hitTest(event.clientX - box.left, event.clientY - box.top, scrollTop, total);
    if (hit) selected = graph.rows[hit.row]?.commit.oid ?? null;
  }

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      viewportHeight = entry.contentRect.height;
      viewportWidth = entry.contentRect.width;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });
</script>

{#if graph.error}
  <p class="message error">{graph.error.message}</p>
{:else if total === 0}
  <p class="message">{graph.loading ? "Reading history…" : "No commits yet."}</p>
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scroll" bind:this={scroller} {onscroll} {onclick}>
    <div class="canvas-layer">
      <GraphCanvas
        edges={graph.edges}
        {nodes}
        {scrollTop}
        width={gutter}
        height={viewportHeight}
        firstRow={range.start}
        lastRow={range.end}
      />
    </div>

    <div class="rows" style:height="{total * GRAPH.rowHeight}px">
      {#each visible as item (item.row.commit.oid)}
        <div
          class="row"
          class:selected={selected === item.row.commit.oid}
          style:top="{item.index * GRAPH.rowHeight}px"
          style:padding-left="{gutter}px"
        >
          <span class="summary truncate">{item.row.commit.summary}</span>
          <span class="author truncate">{item.row.commit.authorName}</span>
          <span class="date tabular"
            >{formatCommitDate(item.row.commit.timestamp, item.row.commit.tzOffsetMinutes)}</span
          >
          <span class="oid mono tabular">{shortOid(item.row.commit.oid)}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .scroll {
    position: relative;
    height: 100%;
    overflow: auto;
  }

  /* Height zero so the pinned canvas claims no layout space of its own. */
  .canvas-layer {
    position: sticky;
    top: 0;
    height: 0;
    z-index: 2;
  }

  .rows {
    position: relative;
    z-index: 1;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: 22px;
    padding-right: var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .author {
    flex: 0 0 auto;
    max-width: 140px;
    color: var(--text-secondary);
  }

  .date,
  .oid {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .error {
    color: var(--status-delete);
    user-select: text;
  }
</style>
