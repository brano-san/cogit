<script lang="ts">
  import GraphCanvas from "$components/graph/GraphCanvas.svelte";
  import { formatCommitDate, refLabels, shortOid } from "$lib/format";
  import {
    GRAPH,
    HEADER_ROWS,
    gutterWidth,
    hitTest,
    toCommitRow,
    visibleRange,
  } from "$lib/graph-geometry";
  import { graph } from "$stores/graph.svelte";
  import { repository } from "$stores/repository.svelte";

  /** Rows rendered beyond the viewport so a fast scroll does not show blanks. */
  const BUFFER_ROWS = 10;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let viewportWidth = $state(0);
  let selected = $state<string | null>(null);

  const commitCount = $derived(graph.rows.length);
  const listRows = $derived(commitCount + HEADER_ROWS);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, listRows, BUFFER_ROWS),
  );
  const gutter = $derived(gutterWidth(graph.maxLane, viewportWidth || 600));

  const labels = $derived(
    refLabels(
      repository.current?.branches ?? [],
      repository.current?.tags ?? [],
      repository.current?.head,
    ),
  );

  const status = $derived(repository.current?.status);
  const headerLabel = $derived.by(() => {
    if (!status) return "Working Tree";
    const parts: string[] = [];
    if (status.staged > 0) parts.push(`${status.staged} staged`);
    if (status.unstaged > 0) parts.push(`${status.unstaged} modified`);
    if (status.untracked > 0) parts.push(`${status.untracked} untracked`);
    if (status.conflicted > 0) parts.push(`${status.conflicted} conflicted`);
    return parts.length > 0 ? `Working Tree (${parts.join(", ")})` : "Working Tree — clean";
  });

  const visible = $derived.by(() => {
    const from = Math.max(range.start, HEADER_ROWS);
    const rows = [];
    for (let listRow = from; listRow < range.end; listRow++) {
      const commitRow = toCommitRow(listRow);
      const entry = commitRow === null ? undefined : graph.rows[commitRow];
      if (entry) rows.push({ listRow, entry });
    }
    return rows;
  });

  const nodes = $derived(visible.map(({ entry }) => ({
    row: entry.lane.row,
    lane: entry.lane.lane,
    color: entry.lane.color,
    merge: entry.lane.kind === "merge",
    root: entry.lane.kind === "root",
  })));

  function onscroll() {
    if (scroller) scrollTop = scroller.scrollTop;
  }

  function onclick(event: MouseEvent) {
    if (!scroller) return;
    const box = scroller.getBoundingClientRect();
    const hit = hitTest(event.clientX - box.left, event.clientY - box.top, scrollTop, listRows);
    if (!hit) return;
    const commitRow = toCommitRow(hit.row);
    selected = commitRow === null ? null : (graph.rows[commitRow]?.commit.oid ?? null);
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
{:else if commitCount === 0 && !graph.loading}
  <p class="message">No commits yet.</p>
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
        firstRow={Math.max(range.start - HEADER_ROWS, 0)}
        lastRow={range.end}
        rowOffset={HEADER_ROWS}
      />
    </div>

    <div class="rows" style:height="{listRows * GRAPH.rowHeight}px">
      {#if range.start === 0}
        <div class="row header" style:top="0px" style:padding-left="{gutter}px">
          <span class="summary truncate">{headerLabel}</span>
          {#if graph.loading}<span class="date">loading…</span>{/if}
        </div>
      {/if}

      {#each visible as item (item.entry.commit.oid)}
        <div
          class="row"
          class:selected={selected === item.entry.commit.oid}
          style:top="{item.listRow * GRAPH.rowHeight}px"
          style:padding-left="{gutter}px"
        >
          {#each labels.get(item.entry.commit.oid) ?? [] as label (label.text)}
            <span class="capsule {label.kind}">{label.text}</span>
          {/each}
          <span class="summary truncate">{item.entry.commit.summary}</span>
          <span class="author truncate">{item.entry.commit.authorName}</span>
          <span class="date tabular"
            >{formatCommitDate(
              item.entry.commit.timestamp,
              item.entry.commit.tzOffsetMinutes,
            )}</span
          >
          <span class="oid mono tabular">{shortOid(item.entry.commit.oid)}</span>
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
    gap: var(--sp-3);
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

  .row.header .summary {
    color: var(--status-modify);
  }

  .capsule {
    flex: 0 0 auto;
    height: 16px;
    padding: 0 var(--sp-3);
    border: 1px solid;
    border-radius: var(--r-md);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 14px;
  }

  .capsule.head {
    color: var(--c-bg-window);
    background: var(--status-ref);
    border-color: var(--status-ref);
  }

  .capsule.local {
    color: var(--status-ref);
    background: var(--c-branch-bg);
    border-color: var(--status-ref);
  }

  .capsule.remote {
    color: var(--text-secondary);
    background: transparent;
    border-color: var(--field-border);
  }

  .capsule.tag {
    color: var(--status-stash);
    background: var(--c-stash-bg);
    border-color: var(--status-stash);
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
