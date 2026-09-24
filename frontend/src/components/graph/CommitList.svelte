<script lang="ts">
  import { untrack } from "svelte";
  import EmptyState from "$components/common/EmptyState.svelte";
  import SkeletonRows from "$components/common/SkeletonRows.svelte";
  import { settings } from "$stores/settings.svelte";
  import GraphCanvas from "$components/graph/GraphCanvas.svelte";
  import RefCapsule from "$components/graph/RefCapsule.svelte";
  import { capsules, dateTooltip, refLabels, shortOid, type RefLabel } from "$lib/format";
  import { DRAG_TYPE, parseDrag, serialiseDrag } from "$lib/drop-target";
  import { overlapLabel, overlapTooltip } from "$lib/overlap";
  import { overlap } from "$stores/overlap.svelte";
  import {
    GRAPH,
    HEADER_ROWS,
    centreRow,
    hitTest,
    nextRow,
    scrollRowIntoView,
    striped,
    textX,
    toCommitRow,
    visibleRange,
  } from "$lib/graph-geometry";
  import { measurer } from "$lib/timing";
  import { subjectMinWidth } from "$lib/graph-panel";
  import { workingTreeLabel } from "$lib/repo-state";
  import { reportTiming, type RebaseProgress, type RepoId } from "$lib/ipc";
  import Avatar from "$components/common/Avatar.svelte";
  import { avatars } from "$stores/avatars.svelte";
  import { commit as selection } from "$stores/commit.svelte";
  import { compareView } from "$stores/compare-view.svelte";
  import { graph } from "$stores/graph.svelte";
  import {
    GRAPH_MODE_DEFAULTS,
    checkedTips,
    focusLane,
    graphView,
    paintRequest,
    type LanePick,
  } from "$lib/graph-modes";
  import { laneAt } from "$lib/graph-style";
  import { isEmptyQuery } from "$lib/query";
  import { graphOverlays } from "$stores/graph-overlay.svelte";
  import { refs as refTicks } from "$stores/refs.svelte";
  import { repository } from "$stores/repository.svelte";
  import { stashes } from "$stores/stashes.svelte";
  import { worktrees } from "$stores/worktrees.svelte";

  interface Props {
    /** Rows of this list, not a block above it: a rebase in flight is part of the history
        the user is reading, and the graph has to draw lines into it. */
    rebase?: RebaseProgress | null;
    /** A commit was dropped on another commit; the caller offers squash or reorder. */
    ondrop?: (source: string, target: string) => void;
    oncontext?: (oid: string, x: number, y: number) => void;
    onworktreecontext?: (x: number, y: number) => void;
    onrefcontext?: (label: RefLabel, oid: string, x: number, y: number) => void;
    /** Branches ticked in Branches in their own colours (setting `graphHighlightChecked`). */
    highlightChecked?: boolean;
    /** First parents only (`graphFirstParent`). */
    firstParent?: boolean;
    /** A click on a commit or its line brings its branch forward (`graphBranchOfCommit`). */
    branchOfCommit?: boolean;
  }

  let {
    rebase = null,
    ondrop,
    oncontext,
    onworktreecontext,
    onrefcontext,
    highlightChecked = GRAPH_MODE_DEFAULTS.highlightChecked,
    firstParent = GRAPH_MODE_DEFAULTS.firstParent,
    branchOfCommit = GRAPH_MODE_DEFAULTS.branchOfCommit,
  }: Props = $props();

  const modes = $derived({ highlightChecked, firstParent, branchOfCommit });
  $effect(() => {
    const view = graphView(modes);
    untrack(() => graph.setView(view));
  });

  /** The other end of a comparison stays marked while the graph shows it (#33). */
  const comparedFrom = $derived(compareView.showing(selection.oid) ? compareView.from : null);

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
        label: step.summary || shortOid(step.oid),
        detail: step.action,
      })),
      {
        kind: "onto",
        label: `Replaying onto ${rebase.onto ? shortOid(rebase.onto) : "the new base"}`,
        detail: `${rebase.done} of ${rebase.total} done`,
      },
    ];
    return rows;
  });

  const headerRows = $derived(HEADER_ROWS + virtualRows.length);
  const commitCount = $derived(graph.total);
  const listRows = $derived(commitCount + headerRows);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, listRows, BUFFER_ROWS),
  );
  /** The Working Tree row and the rebase rows start where HEAD's line is. */
  const headLane = $derived(graph.rowAt(0)?.layout.lane ?? null);
  const headerX = $derived(textX((headLane ?? 0) + 1));

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
      { stashes: stashes.entries, worktrees: worktrees.entries },
    ),
  );
  const stashOids = $derived(new Set(stashes.entries.map((entry) => entry.oid)));

  const headerLabel = $derived(workingTreeLabel(repository.current?.status, repository.current?.state));

  const visible = $derived.by(() => {
    const from = Math.max(range.start, headerRows);
    const rows = [];
    for (let listRow = from; listRow < range.end; listRow++) {
      const commitRow = toCommitRow(listRow, headerRows);
      const entry = commitRow === null ? undefined : graph.rowAt(commitRow);
      if (entry) rows.push({ listRow, entry });
    }
    return rows;
  });

  /** Only the rows on screen come over from Rust (R-193). */
  $effect(() => {
    graph.show(Math.max(range.start - headerRows, 0), Math.max(range.end - headerRows, 0));
  });

  /** A filtered list is flat, not a graph (R-51): nothing to colour along it. */
  const paint = $derived(
    isEmptyQuery(graph.query)
      ? paintRequest(modes, checkedTips(repository.current?.branches ?? [], refTicks.visible))
      : null,
  );
  $effect(() => {
    const walk = graph.walk;
    graphOverlays.show({
      repo: walk?.repo ?? null,
      generation: walk?.generation ?? null,
      start: Math.max(range.start - headerRows, 0),
      end: Math.max(range.end - headerRows, 0),
      total: graph.total,
      complete: graph.complete,
      request: paint,
    });
  });

  let lanePick = $state<LanePick | null>(null);
  const selectedLane = $derived.by(() => {
    void graph.walk;
    const at = graph.loadedIndexOf(selection.oid);
    return at === null ? null : (graphOverlays.paintAt(at)?.nodeLane ?? null);
  });
  const focus = $derived(focusLane(modes, selection.oid, selectedLane, lanePick));

  const drawn = $derived(
    visible.map(({ listRow, entry }) => ({
      listRow,
      layout: entry.layout,
      stash: stashOids.has(entry.commit.oid),
      paint: graphOverlays.paintAt(entry.layout.row),
    })),
  );
  /** The canvas only has to reach the widest row on screen. */
  const canvasWidth = $derived(
    Math.max(headerX, ...drawn.map(({ layout }) => textX(layout.width))),
  );
  const selectedRow = $derived(visible.find(({ entry }) => entry.commit.oid === selection.oid)?.listRow ?? null);

  /** Selection and scroll move together: an arrow key that selects off-screen is useless. */
  function onkeydown(event: KeyboardEvent) {
    const id = repository.current?.repo;
    if (!id) return;

    const at = graph.loadedIndexOf(selection.oid);
    const page = Math.max(Math.floor(viewportHeight / GRAPH.rowHeight) - 1, 1);
    const target = nextRow(at, event.key, graph.total, page);
    if (target === null) return;

    event.preventDefault();
    void graph.entry(target).then((row) => row && pick(id, row.commit.oid));

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

    // Read so that a commit the walk has not reached yet is looked for again.
    void graph.total;
    void graph.indexOf(wanted.oid).then((at) => {
      if (at === null || !scroller || graph.reveal !== wanted || revealed === wanted.request) return;
      scroller.scrollTop = centreRow(at + headerRows, viewportHeight, GRAPH.rowHeight, listRows);
      revealed = wanted.request;
    });
  });

  /** The canvas fills a node with what is behind it, and hover is behind it too. */
  let hoverRow = $state<number | null>(null);
  function onpointermove(event: PointerEvent) {
    if (!scroller) return;
    const box = scroller.getBoundingClientRect();
    hoverRow = hitTest(event.clientX - box.left, event.clientY - box.top, scrollTop, listRows)?.row ?? null;
  }

  function onclick(event: MouseEvent) {
    if (!scroller) return;
    const box = scroller.getBoundingClientRect();
    const hit = hitTest(event.clientX - box.left, event.clientY - box.top, scrollTop, listRows);
    if (!hit) return;
    const repo = repository.current?.repo;
    if (!repo) return;
    const commitRow = toCommitRow(hit.row, headerRows);
    const oid = commitRow === null ? null : (graph.rowAt(commitRow)?.commit.oid ?? null);
    const layout = commitRow === null ? undefined : graph.rowAt(commitRow)?.layout;
    if (branchOfCommit && oid !== null && layout && commitRow !== null) {
      const upper = (event.clientY - box.top + scrollTop) % GRAPH.rowHeight < GRAPH.rowHeight / 2;
      const lane = laneAt(layout, graphOverlays.paintAt(commitRow), hit.lane, upper);
      lanePick = lane === null ? null : { oid, lane };
    }
    // Clicking the selected commit again brings its details back into Diff (#7).
    if (oid !== null && oid === selection.oid) selection.showDetails();
    else void pick(repo, oid);
  }

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      viewportHeight = entry.contentRect.height;
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
    {onpointermove}
    onpointerleave={() => (hoverRow = null)}
    {onkeydown}
    role="listbox"
    aria-label="Commits"
    tabindex="0"
  >
    <div class="viewport">
      <div class="canvas-layer">
        <GraphCanvas
          rows={drawn}
          {scrollTop}
          width={canvasWidth}
          height={viewportHeight}
          firstCommitRow={headerRows}
          {headLane}
          {selectedRow}
          {hoverRow}
          focusLane={focus}
        />
      </div>

      <div
        class="rows"
        style:transform="translateY({-scrollTop}px)"
        style:--row-h="{GRAPH.rowHeight}px"
        style:--subject-min={subjectMinWidth()}
      >
        {#if range.start === 0}
          <button
            type="button"
            class="row header"
            class:selected={selection.oid === null}
            style:top="0px"
            style:padding-left="{headerX}px"
            title="Show the working tree in Files and Diff"
            onclick={() => selection.showWorkingTree()}
            oncontextmenu={(event) => {
              if (!onworktreecontext) return;
              event.preventDefault();
              selection.showWorkingTree();
              onworktreecontext(event.clientX, event.clientY);
            }}
          >
            <span class="summary truncate">{headerLabel}</span>
            {#if graph.loading}<span class="date">loading…</span>{/if}
          </button>
        {/if}

        {#each virtualRows as row, index (index)}
          <div
            class="row virtual {row.kind}"
            class:striped={striped(HEADER_ROWS + index)}
            style:top="{(HEADER_ROWS + index) * GRAPH.rowHeight}px"
            style:padding-left="{headerX}px"
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
            class:striped={striped(item.listRow)}
            class:selected={selection.oid === item.entry.commit.oid || comparedFrom === item.entry.commit.oid}
            class:over={over === item.entry.commit.oid}
            style:top="{item.listRow * GRAPH.rowHeight}px"
            style:padding-left="{textX(item.entry.layout.width)}px"
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
              const repo = repository.current?.repo;
              if (repo !== undefined) void pick(repo, item.entry.commit.oid);
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
              <RefCapsule
                {label}
                onmenu={onrefcontext &&
                  ((event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    const oid = item.entry.commit.oid;
                    const repo = repository.current?.repo;
                    if (repo !== undefined) void pick(repo, oid);
                    onrefcontext(label, oid, event.clientX, event.clientY);
                  })}
              />
            {/each}
            {#if refs.hidden.length > 0}
              <span class="capsule more" title={refs.hidden.map((l) => l.text).join("\n")}
                >+{refs.hidden.length}</span
              >
            {/if}
            <span class="summary truncate">{item.entry.commit.summary}</span>
            <span class="author truncate">{item.entry.commit.authorName}</span>
            <Avatar
              name={item.entry.commit.authorName}
              email={item.entry.commit.authorEmail}
            />
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
    <div
      class="spacer"
      style:height="{Math.max(listRows * GRAPH.rowHeight - viewportHeight, 0)}px"
    ></div>
  </div>
{/if}

<style>
  .scroll {
    position: relative;
    height: 100%;
    overflow: auto;
  }

  /* Text and graph move together, in the frame that draws the graph. Rows scrolled by the
     browser itself ran ahead of the canvas and showed rings with no text beside them. */
  .viewport {
    position: sticky;
    top: 0;
    height: 100%;
    overflow: clip;
  }

  .canvas-layer {
    position: absolute;
    top: 0;
    left: 0;
    z-index: 2;
  }

  .rows {
    position: absolute;
    inset: 0;
    z-index: 1;
    will-change: transform;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--row-h);
    padding-right: var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  /* Before hover and selection, which cover it. */
  .row.striped {
    background: var(--row-stripe);
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

  /* Kept, not squeezed to nothing: past it the row is cut by the panel's edge (#5). */
  .summary {
    flex: 1 1 auto;
    min-width: var(--subject-min, 0);
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
