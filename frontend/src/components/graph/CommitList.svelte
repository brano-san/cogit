<script lang="ts">
  import EmptyState from "$components/common/EmptyState.svelte";
  import SkeletonRows from "$components/common/SkeletonRows.svelte";
  import { settings } from "$stores/settings.svelte";
  import GraphCanvas from "$components/graph/GraphCanvas.svelte";
  import { capsules, dateTooltip, refLabels, shortOid } from "$lib/format";
  import { DRAG_TYPE, parseDrag, serialiseDrag } from "$lib/drop-target";
  import { overlapLabel, overlapTooltip } from "$lib/overlap";
  import { overlap } from "$stores/overlap.svelte";
  import {
    GRAPH,
    HEADER_ROWS,
    centreRow,
    gutterWidth,
    hitTest,
    nextRow,
    scrollRowIntoView,
    toCommitRow,
    visibleRange,
  } from "$lib/graph-geometry";
  import { measurer } from "$lib/timing";
  import { reportTiming, type RebaseProgress, type RepoId } from "$lib/ipc";
  import { avatars } from "$stores/avatars.svelte";
  import { commit as selection } from "$stores/commit.svelte";
  import { graph } from "$stores/graph.svelte";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** Rows of this list, not a block above it: a rebase in flight is part of the history
        the user is reading, and the graph has to draw lines into it. */
    rebase?: RebaseProgress | null;
    /** A commit was dropped on another commit; the caller offers squash or reorder. */
    ondrop?: (source: string, target: string) => void;
    oncontext?: (oid: string, x: number, y: number) => void;
    onref?: (text: string) => void;
  }

  let { rebase = null, ondrop, oncontext, onref }: Props = $props();

  let over = $state<string | null>(null);

  /** Answers "why did the panel below take so long?" in the log the user sends back. */
  const measure = measurer((label, ms, detail) => void reportTiming(label, ms, detail));

  async function pick(repo: RepoId, oid: string | null) {
    const watch = measure("select-commit");
    await selection.select(repo, oid);
    watch.stop(`${selection.files.length} files`);
  }

  /** Enough for HEAD plus its upstream plus a tag; the rest fold into a `+N` capsule. */
  const CAPSULE_ROOM = 3;

  /** Rows rendered beyond the viewport so a fast scroll does not show blanks. */
  const BUFFER_ROWS = 10;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let viewportWidth = $state(0);

  /** Index and every pending step, sitting between Working Tree and the first commit. */
  const virtualRows = $derived.by(() => {
    if (!rebase) return [];
    const staged = repository.current?.status?.staged ?? 0;
    const rows = [
      {
        kind: "index",
        label: "Index",
        detail: rebase.applying ? `rebasing: ${rebase.applying}` : `${staged} staged`,
      },
      ...rebase.todo.map((step) => ({
        kind: "todo",
        label: step.summary || step.oid.slice(0, 7),
        detail: step.action,
      })),
      {
        kind: "onto",
        label: `Replaying onto ${rebase.onto ? rebase.onto.slice(0, 7) : "the new base"}`,
        detail: `${rebase.done} of ${rebase.total} done`,
      },
    ];
    return rows;
  });

  const headerRows = $derived(HEADER_ROWS + virtualRows.length);
  const commitCount = $derived(graph.rows.length);
  const listRows = $derived(commitCount + headerRows);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, listRows, BUFFER_ROWS),
  );
  const gutter = $derived(gutterWidth(graph.maxLane, viewportWidth || 600));

  /** The window drives the queue: rows that scroll away stop being asked for. */
  $effect(() => {
    if (!avatars.enabled) return;
    void avatars.load(visible.map((item) => item.entry.commit));
  });

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
    const from = Math.max(range.start, headerRows);
    const rows = [];
    for (let listRow = from; listRow < range.end; listRow++) {
      const commitRow = toCommitRow(listRow, headerRows);
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
    void pick(id, row.commit.oid);

    const offset = scrollRowIntoView(
      target + headerRows,
      scrollTop,
      viewportHeight,
      GRAPH.rowHeight,
    );
    if (offset !== null && scroller) scroller.scrollTop = offset;
  }

  function onscroll() {
    if (scroller) scrollTop = scroller.scrollTop;
  }

  /** The request already acted on. Without it every arriving chunk would re-centre and
      fight the user's own scrolling; a commit not on screen yet keeps the request open. */
  let revealed = $state(-1);

  /** A commit reached from the References panel is centred, not merely brought on screen:
      arriving from elsewhere, the user needs the rows around it to know where they are. */
  $effect(() => {
    const wanted = graph.reveal;
    if (!wanted) {
      revealed = -1;
      return;
    }
    if (!scroller || wanted.request === revealed) return;

    const at = graph.rows.findIndex((row) => row.commit.oid === wanted.oid);
    if (at < 0) return;

    scroller.scrollTop = centreRow(at + headerRows, viewportHeight, GRAPH.rowHeight, listRows);
    revealed = wanted.request;
  });

  function onclick(event: MouseEvent) {
    if (!scroller) return;
    const box = scroller.getBoundingClientRect();
    const hit = hitTest(event.clientX - box.left, event.clientY - box.top, scrollTop, listRows);
    if (!hit) return;
    const repo = repository.current?.repo;
    if (!repo) return;
    const commitRow = toCommitRow(hit.row, headerRows);
    void pick(repo, commitRow === null ? null : (graph.rows[commitRow]?.commit.oid ?? null));
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
{:else if commitCount === 0 && graph.loading}
  <SkeletonRows rows={14} />
{:else if commitCount === 0}
  <EmptyState
    title="No commits yet"
    hint="The first commit you make in this repository shows up here."
  />
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
        firstRow={Math.max(range.start - headerRows, 0)}
        lastRow={range.end}
        rowOffset={headerRows}
        headLane={graph.rows[0]?.lane.lane ?? null}
      />
    </div>

    <div class="rows" style:height="{listRows * GRAPH.rowHeight}px">
      {#if range.start === 0}
        <button
          type="button"
          class="row header"
          class:selected={selection.oid === null}
          style:top="0px"
          style:padding-left="{gutter}px"
          title="Show the working tree in Files and Diff"
          onclick={() => selection.clear()}
        >
          <span class="summary truncate">{headerLabel}</span>
          {#if graph.loading}<span class="date">loading…</span>{/if}
        </button>
      {/if}

      {#each virtualRows as row, index (index)}
        <div
          class="row virtual {row.kind}"
          style:top="{(HEADER_ROWS + index) * GRAPH.rowHeight}px"
          style:padding-left="{gutter}px"
        >
          <span class="node" aria-hidden="true">{row.kind === "onto" ? "▶" : "◌"}</span>
          <span class="summary truncate">{row.label}</span>
          <span class="date">{row.detail}</span>
        </div>
      {/each}

      {#each visible as item (item.entry.commit.oid)}
        {@const refs = capsules(labels.get(item.entry.commit.oid) ?? [], CAPSULE_ROOM)}
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
            void pick(repository.current?.repo ?? (0 as unknown as RepoId), item.entry.commit.oid);
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
          {#each refs.shown as label (label.text)}
            <span
              class="capsule {label.kind}"
              role="button"
              tabindex="-1"
              title={label.text}
              onclick={(event) => {
                event.stopPropagation();
                onref?.(label.text);
              }}
              onkeydown={(event) => event.key === "Enter" && onref?.(label.text)}
            >{label.text}</span>
          {/each}
          {#if refs.hidden.length > 0}
            <span class="capsule more" title={refs.hidden.map((l) => l.text).join("\n")}
              >+{refs.hidden.length}</span
            >
          {/if}
          <span class="summary truncate">{item.entry.commit.summary}</span>
          {#if avatars.enabled}
            {@const face = avatars.look(item.entry.commit.authorEmail)}
            <span
              class="avatar"
              style:background={face?.image ? "transparent" : (face?.color ?? "var(--surface-raised)")}
              title={item.entry.commit.authorEmail}
            >
              {#if face?.image}
                <img src={face.image} alt="" width="16" height="16" />
              {:else}
                {face?.initials ?? ""}
              {/if}
            </span>
          {/if}
          <span class="author truncate">{item.entry.commit.authorName}</span>
          <span
            class="date tabular"
            title={dateTooltip(
              item.entry.commit.timestamp,
              item.entry.commit.tzOffsetMinutes,
            )}>{settings.formatDate(
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

  /* A bar as well as a tint: two greys apart is not something everyone can see. */
  .row.selected {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .row.selected.over {
    box-shadow:
      inset 2px 0 0 var(--status-ref),
      inset 0 0 0 1px var(--status-ref);
  }

  /* A button, so it needs the row geometry rather than the browser default. */
  button.row {
    width: 100%;
    background: none;
    color: inherit;
    border: 0;
    font: inherit;
    text-align: left;
  }

  .row.virtual {
    color: var(--text-secondary);
  }

  .row.virtual .node {
    flex: 0 0 12px;
    color: var(--status-modify);
    font-size: 9px;
    text-align: center;
  }

  .row.virtual.onto .node {
    color: var(--status-ref);
  }

  .row.virtual.todo .node {
    color: var(--text-secondary);
  }

  .row.header .summary {
    color: var(--status-modify);
  }

  .capsule.more {
    background: var(--surface-raised);
    color: var(--text-secondary);
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

  /* Fixed width so the author column does not shift as pictures arrive. */
  .avatar {
    flex: 0 0 16px;
    width: 16px;
    height: 16px;
    border-radius: var(--r-sm);
    overflow: hidden;
    color: var(--text-on-accent);
    font-size: 9px;
    font-weight: 600;
    line-height: 16px;
    text-align: center;
  }

  .avatar img {
    display: block;
    width: 16px;
    height: 16px;
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
