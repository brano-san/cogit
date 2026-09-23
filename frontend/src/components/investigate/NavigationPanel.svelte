<script lang="ts">
  import VirtualList from "$components/common/VirtualList.svelte";
  import { dateOf, shortAuthor } from "$lib/investigate/blame";
  import { segmentsOf, type GraphRow } from "$lib/investigate/graph";
  import type { NavItem } from "$lib/investigate/navigation";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";

  interface Props {
    session: InvestigateSession;
  }

  let { session }: Props = $props();

  const ROW = 22;
  const LANE = 12;
  /** The Working Tree row sits above the newest commit and leads into it. */
  const WORKING_TREE: GraphRow = {
    lane: 0,
    through: [],
    fromAbove: false,
    joins: [],
    toParents: [0],
    width: 1,
  };

  const lanes = $derived(
    Math.max(1, ...session.items.map((item) => (item.kind === "commit" ? item.graph.width : 1))),
  );
  const reveal = $derived(session.selectedItem >= 0 ? session.selectedItem : null);

  function color(lane: number): string {
    const style = getComputedStyle(document.documentElement);
    return style.getPropertyValue(`--c-lane-${(lane % 8) + 1}`).trim() || "#888";
  }

  function draw(canvas: HTMLCanvasElement, { row, width, hollow }: Drawn) {
    const ratio = window.devicePixelRatio || 1;
    canvas.width = width * LANE * ratio;
    canvas.height = ROW * ratio;
    canvas.style.width = `${width * LANE}px`;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.scale(ratio, ratio);
    const { node, lines } = segmentsOf(row, LANE, ROW);
    context.lineWidth = 1.5;
    context.strokeStyle = getComputedStyle(canvas).getPropertyValue("--graph-line").trim() || "#888";
    for (const [from, to] of lines) {
      context.beginPath();
      context.moveTo(from[0], from[1]);
      context.lineTo(to[0], to[1]);
      context.stroke();
    }
    context.beginPath();
    context.arc(node[0], node[1], 4, 0, Math.PI * 2);
    context.fillStyle = color(row.lane);
    context.strokeStyle = color(row.lane);
    if (hollow) context.stroke();
    else context.fill();
  }

  interface Drawn {
    row: GraphRow;
    width: number;
    hollow: boolean;
  }

  function graph(canvas: HTMLCanvasElement, drawn: Drawn) {
    draw(canvas, drawn);
    return { update: (next: Drawn) => draw(canvas, next) };
  }

  function move(step: 1 | -1) {
    let at = session.selectedItem + step;
    while (session.items[at]?.kind === "header") at += step;
    if (at >= 0 && at < session.items.length) void session.selectItem(at);
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.altKey || event.ctrlKey) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      move(event.key === "ArrowDown" ? 1 : -1);
    }
  }

  function renamed(item: Extract<NavItem, { kind: "commit" }>): string | null {
    if (item.row.change === "renamed" && item.row.previousPath) return `renamed from ${item.row.previousPath}`;
    if (item.row.path && item.row.path !== item.path) return `as ${item.row.path}`;
    return null;
  }
</script>

<div class="panel">
  <header>
    <span class="title">Navigation</span>
    {#if session.logError}<span class="error truncate">{session.logError}</span>{/if}
  </header>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="list" tabindex="0" role="listbox" aria-label="Commits that changed the file" {onkeydown}>
    <VirtualList items={session.items} rowHeight={ROW} {reveal}>
      {#snippet row(item: NavItem, index: number)}
        {#if item.kind === "header"}
          <div class="row header" style:top="{index * ROW}px" title={item.path}>
            <svg viewBox="0 0 16 16" aria-hidden="true"
              ><path d="M4 1.5h5l3 3v10H4z M9 1.5v3h3" /></svg
            >
            <span class="truncate">{item.path}</span>
          </div>
        {:else}
          {@const selected = index === session.selectedItem}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="row"
            class:selected
            role="option"
            aria-selected={selected}
            tabindex="-1"
            style:top="{index * ROW}px"
            onclick={() => void session.selectItem(index)}
          >
            {#if item.kind === "workingTree"}
              <span class="hash"></span>
              <canvas
                class="graph"
                height={ROW}
                use:graph={{ row: WORKING_TREE, width: lanes, hollow: true }}
              ></canvas>
              <span class="summary working">Working Tree</span>
              <span class="author"></span>
              <span class="date muted">uncommitted</span>
            {:else}
              {@const note = renamed(item)}
              <span class="hash mono">{item.row.oid.slice(0, 7)}</span>
              <canvas
                class="graph"
                height={ROW}
                use:graph={{ row: item.graph, width: lanes, hollow: item.row.parents.length > 1 }}
              ></canvas>
              <span class="summary truncate" title={item.row.summary}
                >{item.row.summary}{#if note}<span class="note">{note}</span>{/if}</span
              >
              <span class="author truncate" title="{item.row.author} <{item.row.email}>"
                >{shortAuthor(item.row.author)}</span
              >
              <span class="date tabular">{dateOf(item.row.timestamp)}</span>
            {/if}
          </div>
        {/if}
      {/snippet}
    </VirtualList>
  </div>
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--surface-base);
  }

  header {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-5);
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-header);
  }

  .title {
    font-weight: 600;
  }

  .error {
    color: var(--status-delete);
  }

  .list {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
    outline: none;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .row:not(.header):hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .header {
    gap: var(--sp-3);
    background: var(--surface-panel);
    font-weight: 600;
  }

  .header svg {
    flex: none;
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
  }

  .hash {
    flex: 0 0 7ch;
    color: var(--text-secondary);
  }

  .graph {
    flex: none;
    height: 22px;
  }

  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .working {
    font-style: italic;
  }

  .note {
    margin-left: var(--sp-4);
    color: var(--text-secondary);
  }

  .author {
    flex: 0 0 11ch;
    color: var(--text-secondary);
  }

  .date {
    flex: 0 0 10ch;
    color: var(--text-secondary);
    text-align: right;
  }
</style>
