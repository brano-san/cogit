<script lang="ts">
  import { settings } from "$stores/settings.svelte";
  import GraphCanvas from "$components/graph/GraphCanvas.svelte";
  import { refLabels, shortOid } from "$lib/format";
  import { DRAG_TYPE, parseDrag, serialiseDrag } from "$lib/drop-target";
  import { overlapLabel, overlapTooltip } from "$lib/overlap";
  import { overlap } from "$stores/overlap.svelte";
  import {
    GRAPH,
    HEADER_ROWS,
    gutterWidth,
    hitTest,
    nextRow,
    scrollRowIntoView,
    toCommitRow,
    visibleRange,
  } from "$lib/graph-geometry";
  import { commit as selection } from "$stores/commit.svelte";
  import { graph } from "$stores/graph.svelte";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** A commit was dropped on another commit; the caller offers squash or reorder. */
    ondrop?: (source: string, target: string) => void;
    oncontext?: (oid: string, x: number, y: number) => void;
  }

  let { ondrop, oncontext }: Props = $props();

  let over = $state<string | null>(null);

  /** Rows rendered beyond the viewport so a fast scroll does not show blanks. */
  const BUFFER_ROWS = 10;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let viewportWidth = $state(0);

  const commitCount = $derived(graph.rows.length);
  const listRows = $derived(commitCount + HEADER_ROWS);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, listRows, BUFFER_ROWS),
  );
  const gutter = $derived(gutterWidth(graph.maxLane, viewportWidth || 600));

  $effect(() => {
    const id = repository.current?.repo;
    const base = selection.oid;
    if (!id || !base || !overlap.enabled) return;
    void overlap.load(
      id,
      base,
      visible.map((item) => item.entry.commit.oid),
    );
  });

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

  /** Selection and scroll move together: an arrow key that selects off-screen is useless. */
  function onkeydown(event: KeyboardEvent) {
    const id = repository.current?.repo;
    if (!id) return;

    const at = graph.rows.findIndex((row) => row.commit.oid === selection.oid);
    const page = Math.max(Math.floor(viewportHeight / GRAPH.rowHeight) - 1, 1);
    const target = nextRow(at < 0 ? null : at, event.key, graph.rows.length, page);
    if (target === null) return;

    event.preventDefault();
    const row = graph.rows[target];
    if (!row) return;
    void selection.select(id, row.commit.oid);

    const offset = scrollRowIntoView(
      target + HEADER_ROWS,
      scrollTop,
      viewportHeight,
      GRAPH.rowHeight,
    );
    if (offset !== null && scroller) scroller.scrollTop = offset;
  }

  function onscroll() {
    if (scroller) scrollTop = scroller.scrollTop;
  }

  function onclick(event: MouseEvent) {
    if (!scroller) return;
    const box = scroller.getBoundingClientRect();
    const hit = hitTest(event.clientX - box.left, event.clientY - box.top, scrollTop, listRows);
    if (!hit) return;
    const repo = repository.current?.repo;
    if (!repo) return;
    const commitRow = toCommitRow(hit.row);
    void selection.select(repo, commitRow === null ? null : (graph.rows[commitRow]?.commit.oid ?? null));
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
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <div
    class="scroll"
    bind:this={scroller}
    {onscroll}
    {onclick}
    {onkeydown}
    role="listbox"
    aria-label="Commits"
    tabindex="0"
  >
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
          class:selected={selection.oid === item.entry.commit.oid}
          class:over={over === item.entry.commit.oid}
          style:top="{item.listRow * GRAPH.rowHeight}px"
          style:padding-left="{gutter}px"
          role="listitem"
          draggable={ondrop !== undefined}
          ondragstart={(event) =>
            event.dataTransfer?.setData(
              DRAG_TYPE,
              serialiseDrag({ kind: "commit", id: item.entry.commit.oid }),
            )}
          ondragover={(event) => {
            if (ondrop) {
              event.preventDefault();
              over = item.entry.commit.oid;
            }
          }}
          ondragleave={() => (over = null)}
          oncontextmenu={(event) => {
            if (!oncontext) return;
            event.preventDefault();
            void selection.select(repository.current?.repo ?? 0, item.entry.commit.oid);
            oncontext(item.entry.commit.oid, event.clientX, event.clientY);
          }}
          ondrop={(event) => {
            over = null;
            const payload = parseDrag(event.dataTransfer?.getData(DRAG_TYPE) ?? "");
            if (payload?.kind === "commit" && payload.id !== item.entry.commit.oid) {
              ondrop?.(payload.id, item.entry.commit.oid);
            }
          }}
        >
          {#each labels.get(item.entry.commit.oid) ?? [] as label (label.text)}
            <span class="capsule {label.kind}">{label.text}</span>
          {/each}
          <span class="summary truncate">{item.entry.commit.summary}</span>
          <span class="author truncate">{item.entry.commit.authorName}</span>
          <span class="date tabular"
            >{settings.formatDate(
              item.entry.commit.timestamp,
              item.entry.commit.tzOffsetMinutes,
            )}</span
          >
          {#if overlap.enabled}
            {@const row = overlap.rows.get(item.entry.commit.oid)}
            <span
              class="overlap {row?.overlap ?? 'none'}"
              class:base={row?.isBase}
              title={row ? overlapTooltip(row.shared, row.sharedTotal) : ""}
            >
              {row?.isBase ? "base" : row ? overlapLabel(row.overlap) : ""}
            </span>
          {/if}
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

  .row.over {
    box-shadow: inset 0 0 0 1px var(--status-ref);
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
  .overlap {
    flex: 0 0 74px;
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-align: right;
  }

  .overlap.heavy {
    color: var(--status-modify);
  }

  .overlap.same {
    color: var(--status-delete);
  }

  .overlap.base {
    color: var(--status-ref);
    font-weight: 600;
  }

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
