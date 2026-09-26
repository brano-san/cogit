<script lang="ts">
  import { untrack } from "svelte";
  import EmptyState from "$components/common/EmptyState.svelte";
  import SkeletonRows from "$components/common/SkeletonRows.svelte";
  import GraphCanvas from "$components/graph/GraphCanvas.svelte";
  import RefCapsule from "$components/graph/RefCapsule.svelte";
  import { capsules, dateTooltip, refLabelKey, refLabels, shortOid, type RefLabel } from "$lib/format";
  import { graphDropTarget } from "$lib/drop-target";
  import { pointerDrag } from "$lib/pointer-drag";
  import { overlapLabel, overlapTooltip } from "$lib/overlap";
  import { overlap } from "$stores/overlap.svelte";
  import {
    GRAPH,
    HEADER_ROWS,
    centreRow,
    clickedCommit,
    headNode,
    hitTest,
    keyTarget,
    nextRow,
    scrollRowIntoView,
    setGraphRowHeight,
    striped,
    textX,
    toCommitRow,
    visibleRange,
  } from "$lib/graph-geometry";
  import {
    COLUMN_WIDTH,
    DENSITY_ROW_HEIGHT,
    GRAPH_COLUMNS,
    GRAPH_DENSITY,
    GRAPH_STRIPES,
    GRAPH_TIME_FORMAT,
    LONG_LINK_ROWS,
    graphClipX,
    graphTime,
    rightCells,
    rightColumnsWidth,
    rowTextX,
    timeWidth,
    type GraphColumn,
    type GraphDensity,
    type GraphTimeFormat,
  } from "$lib/graph-row";
  import { linkStubs, linkTitle } from "$lib/graph-links";
  import { measurer } from "$lib/timing";
  import { anchoredScrollTop } from "$lib/graph-anchor";
  import { emptyHistory, subjectRoom } from "$lib/graph-panel";
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
    effectiveModes,
    focusLane,
    graphView,
    paintRequest,
    type LanePick,
  } from "$lib/graph-modes";
  import FoldToggle from "$components/graph/FoldToggle.svelte";
  import { graphFolds } from "$stores/graph-folds.svelte";
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
    /** A commit was dropped on another commit; the caller offers squash or reorder where
        the pointer was released. */
    ondrop?: (source: string, target: string, x: number, y: number) => void;
    oncontext?: (oid: string, x: number, y: number) => void;
    onworktreecontext?: (x: number, y: number) => void;
    onrefcontext?: (label: RefLabel, oid: string, x: number, y: number) => void;
    /** A filter left the list empty; its button clears the filter. */
    onclearfilter?: () => void;
    /** Branches ticked in Branches in their own colours (setting `graphHighlightChecked`). */
    highlightChecked?: boolean;
    /** First parents only (`graphFirstParent`). */
    firstParent?: boolean;
    /** A click on a commit or its line brings its branch forward (`graphBranchOfCommit`). */
    branchOfCommit?: boolean;
    /** The chosen commit's ancestors and descendants stand out (`graphAncestry`). */
    ancestry?: boolean;
    /** A merged branch folds into its merge row (`graphCollapseMerged`). */
    collapseMerged?: boolean;
    /** The right columns shown, in order (#12). The defaults are the list as it always was. */
    columns?: readonly GraphColumn[];
    timeFormat?: GraphTimeFormat;
    density?: GraphDensity;
    stripes?: boolean;
    /** Links longer than this many rows are two stubs (R-330); 0 draws every link whole. */
    longLinkRows?: number;
  }

  let {
    rebase = null,
    ondrop,
    oncontext,
    onworktreecontext,
    onrefcontext,
    onclearfilter,
    highlightChecked = GRAPH_MODE_DEFAULTS.highlightChecked,
    firstParent = GRAPH_MODE_DEFAULTS.firstParent,
    branchOfCommit = GRAPH_MODE_DEFAULTS.branchOfCommit,
    ancestry = GRAPH_MODE_DEFAULTS.ancestry,
    collapseMerged = GRAPH_MODE_DEFAULTS.collapseMerged,
    columns = GRAPH_COLUMNS,
    timeFormat = GRAPH_TIME_FORMAT,
    density = GRAPH_DENSITY,
    stripes = GRAPH_STRIPES,
    longLinkRows = LONG_LINK_ROWS,
  }: Props = $props();

  const rowHeight = $derived(DENSITY_ROW_HEIGHT[density]);
  const cells = $derived(rightCells(columns, overlap.enabled));

  $effect(() => {
    const rows = longLinkRows;
    untrack(() => graph.setLongLinkRows(rows));
  });

  const modes = $derived(
    effectiveModes({ highlightChecked, firstParent, branchOfCommit, ancestry, collapseMerged }),
  );
  $effect(() => graphFolds.forRepo(repository.current?.repo ?? null));
  $effect(() => {
    const view = graphView(modes, graphFolds.expanded);
    untrack(() => graph.setView(view));
  });

  /** The other end of a comparison stays marked while the graph shows it (#33). */
  const comparedFrom = $derived(compareView.showing(selection.oid) ? compareView.from : null);

  let over = $state<string | null>(null);

  /** Answers "why did the panel below take so long?" in the log the user sends back. */
  const measure = measurer((label, ms, detail) => void reportTiming(label, ms, detail));

  async function pick(repo: RepoId, oid: string | null) {
    // The last repository's rows stay on screen until the new ones arrive (R-300).
    if (graph.stale) return;
    const watch = measure("select-commit");
    await selection.select(repo, oid);
    watch.stop(`${selection.files.length} files`);
  }

  /** Enough for HEAD plus its upstream plus a tag; the rest fold into a `+N` capsule. */
  const CAPSULE_ROOM = 3;

  /** Rows rendered beyond the viewport so a fast scroll does not show blanks. */
  const BUFFER_ROWS = 10;

  let scroller: HTMLDivElement | undefined = $state();
  let rowsLayer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let panelWidth = $state(0);
  /** The row's gap and right padding and one digit of its font, read from the page. */
  let metrics = $state({ gap: 6, padding: 12, char: 7 });

  /** Geometry reads the row height outside Svelte; a new density keeps the top row (#13). */
  let drawnRowHeight = GRAPH.rowHeight;
  $effect.pre(() => setGraphRowHeight(rowHeight));
  $effect(() => {
    const next = rowHeight;
    if (next === drawnRowHeight) return;
    const before = { scrollTop: untrack(() => scrollTop), rowHeight: drawnRowHeight };
    drawnRowHeight = next;
    if (!scroller) return;
    const after = { rowHeight: next, viewportHeight: untrack(() => viewportHeight), totalRows: untrack(() => listRows) };
    scroller.scrollTop = anchoredScrollTop(before, after);
  });

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
    visibleRange(scrollTop, viewportHeight, rowHeight, listRows, BUFFER_ROWS),
  );

  const rightWidth = $derived(
    rightColumnsWidth({
      columns,
      avatars: avatars.enabled,
      time: timeFormat,
      overlap: overlap.enabled,
      gap: metrics.gap,
      padding: metrics.padding,
    }),
  );
  /** Past this the graph area is cut, so the right columns are never pushed out (#12). */
  const clipX = $derived(
    panelWidth > 0 ? graphClipX(panelWidth, rightWidth, subjectRoom(metrics.char)) : Number.POSITIVE_INFINITY,
  );

  const headOid = $derived.by(() => {
    const head = repository.current?.head;
    return head && head.kind !== "unborn" ? head.oid : null;
  });
  const headKey = $derived(`${graph.walk?.generation ?? ""}:${graph.complete}:${headOid ?? ""}`);
  const headNear = $derived(graph.loadedIndexOf(headOid));
  /** HEAD far below the loaded rows, asked for once per walk and once more when it ends. */
  let headFar = $state.raw<{ key: string; row: number; lane: number } | null>(null);
  $effect(() => {
    const oid = headOid;
    const key = headKey;
    if (oid === null || headNear !== null) return;
    void graph.locate(oid).then((place) => {
      headFar = place && { key, ...place };
    });
  });
  const far = $derived(headFar?.key === headKey ? headFar : null);
  const head = $derived(
    headNode(
      headNear ?? far?.row ?? null,
      (row) => graph.rowAt(row)?.layout.lane ?? (row === far?.row ? far.lane : undefined),
      headerRows,
    ),
  );
  /** The Working Tree row and the rebase rows start where HEAD's line is. */
  const headLane = $derived(head?.lane ?? null);
  const headerX = $derived(rowTextX((headLane ?? 0) + 1, clipX));

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
  const empty = $derived(emptyHistory(!isEmptyQuery(graph.query), graph.visibleRefs));

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

  /** Another repository's history opens at its top (R-300). */
  $effect(() => {
    void graph.home;
    if (scroller) scroller.scrollTop = 0;
  });

  /** Only the rows on screen come over from Rust (R-193). */
  $effect(() => {
    graph.show(Math.max(range.start - headerRows, 0), Math.max(range.end - headerRows, 0));
  });

  /** A filtered list is flat, not a graph (R-51): nothing to colour along it. */
  const paint = $derived(
    isEmptyQuery(graph.query)
      ? paintRequest(
          modes,
          checkedTips(repository.current?.branches ?? [], refTicks.visible),
          selection.oid,
        )
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
  const walkKey = $derived(`${graph.walk?.repo ?? ""}:${graph.walk?.generation ?? ""}`);
  const selectedLane = $derived.by(() => {
    void graph.walk;
    const at = graph.loadedIndexOf(selection.oid);
    return at === null ? null : (graphOverlays.paintAt(at)?.nodeLane ?? null);
  });
  const focus = $derived(focusLane(modes, selection.oid, selectedLane, lanePick, walkKey));

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
    Math.min(Math.max(headerX, ...drawn.map(({ layout }) => textX(layout.width))), clipX),
  );
  const selectedRow = $derived(visible.find(({ entry }) => entry.commit.oid === selection.oid)?.listRow ?? null);

  /** Selection and scroll move together: an arrow key that selects off-screen is useless. */
  function onkeydown(event: KeyboardEvent) {
    const id = repository.current?.repo;
    if (!id) return;

    const page = Math.max(Math.floor(viewportHeight / rowHeight) - 1, 1);
    if (nextRow(0, event.key, graph.total, page) === null) return;

    event.preventDefault();
    void keyTarget(graph, selection.oid, event.key, graph.total, page).then((target) => {
      if (target === null) return;
      void graph.entry(target).then((row) => row && pick(id, row.commit.oid));
      const offset = scrollRowIntoView(target + headerRows, scrollTop, viewportHeight, rowHeight);
      if (offset !== null && scroller) scroller.scrollTop = offset;
    });
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
      scroller.scrollTop = centreRow(at + headerRows, viewportHeight, rowHeight, listRows);
      revealed = wanted.request;
    });
  });

  function commitAt(clientY: number): string | null {
    if (!scroller || graph.stale) return null;
    const y = clientY - scroller.getBoundingClientRect().top;
    return graphDropTarget(y, scroller.scrollTop, rowHeight, listRows, headerRows, (row) => graph.rowAt(row)?.commit.oid);
  }

  /** A commit dragged onto another (R-450): only from a commit row, never the scrollbar. */
  const commitDrag = $derived(
    ondrop
      ? {
          sourceAt: (event: PointerEvent) =>
            (event.target as Element | null)?.closest(".row:not(.header):not(.virtual)")
              ? commitAt(event.clientY)
              : null,
          targetAt: (_x: number, y: number) => commitAt(y),
          onover: (target: string | null) => (over = target),
          ondrop,
        }
      : null,
  );

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
    const oid = clickedCommit(hit.row, headerRows, (row) => graph.rowAt(row)?.commit.oid);
    if (oid === undefined) return;
    const layout = commitRow === null ? undefined : graph.rowAt(commitRow)?.layout;
    if (branchOfCommit && oid !== null && layout && commitRow !== null) {
      const upper = (event.clientY - box.top + scrollTop) % rowHeight < rowHeight / 2;
      const lane = laneAt(layout, graphOverlays.paintAt(commitRow), hit.lane, upper);
      lanePick = lane === null ? null : { oid, lane, walk: walkKey };
    }
    // Clicking the selected commit again brings its details back into Diff (#7).
    if (oid !== null && oid === selection.oid) selection.showDetails();
    else void pick(repo, oid);
  }

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry || !scroller) return;
      viewportHeight = entry.contentRect.height;
      panelWidth = entry.contentRect.width;
      // The row at the top stays there when the panel changes height (#13).
      const kept = anchoredScrollTop({ scrollTop, rowHeight }, { rowHeight, viewportHeight, totalRows: listRows });
      if (scroller.scrollTop !== kept) scroller.scrollTop = kept;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  $effect(() => {
    if (!rowsLayer) return;
    const style = getComputedStyle(rowsLayer);
    const context = document.createElement("canvas").getContext("2d");
    if (context) context.font = style.font;
    metrics = {
      gap: parseFloat(style.getPropertyValue("--sp-3")) || 6,
      padding: parseFloat(style.getPropertyValue("--sp-5")) || 12,
      char: context?.measureText("0").width || 7,
    };
  });

  /** A stub of a cut link takes the list to the commit at its other end (R-330). */
  function jump(oid: string | undefined) {
    const repo = repository.current?.repo;
    if (!oid || repo === undefined) return;
    graph.requestReveal(oid);
    void pick(repo, oid);
  }

  function describeEnd(oid: string) {
    const at = graph.loadedIndexOf(oid);
    return { short: shortOid(oid), summary: (at === null ? undefined : graph.rowAt(at))?.commit.summary ?? null };
  }

  /** The far ends are rarely on screen; their rows are fetched while the tooltip waits. */
  function prefetch(oids: readonly string[]) {
    for (const oid of oids.slice(0, 6)) {
      if (graph.loadedIndexOf(oid) !== null) continue;
      void graph.indexOf(oid).then((at) => (at === null ? undefined : graph.entry(at)));
    }
  }
</script>

{#if graph.error}
  <p class="message error">{graph.error.message}</p>
{:else if commitCount === 0 && graph.loading}
  <SkeletonRows rows={14} />
{:else}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <div
    class="scroll key-list"
    bind:this={scroller}
    use:pointerDrag={commitDrag}
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
          headRow={head?.listRow ?? null}
          {headLane}
          {selectedRow}
          {hoverRow}
          focusLane={focus}
          {clipX}
          {stripes}
          {rowHeight}
        />
      </div>

      <div
        class="rows"
        bind:this={rowsLayer}
        style:transform="translateY({-scrollTop}px)"
        style:--row-h="{rowHeight}px"
        style:--author-max="{COLUMN_WIDTH.author}px"
        style:--hash-w="{COLUMN_WIDTH.hash}px"
        style:--overlap-w="{COLUMN_WIDTH.overlap}px"
        style:--time-w="{timeWidth(timeFormat)}px"
      >
        {#if range.start === 0}
          <button
            type="button"
            class="row header"
            class:selected={selection.oid === null}
            style:top="0px"
            style:padding-left="{headerX}px"
            title="Show the working tree in Files and Diff"
            onclick={(event) => {
              event.stopPropagation();
              selection.showWorkingTree();
            }}
            oncontextmenu={(event) => {
              if (!onworktreecontext) return;
              event.preventDefault();
              selection.showWorkingTree();
              onworktreecontext(event.clientX, event.clientY);
            }}
          >
            <span class="summary truncate">{headerLabel}</span>
          </button>
        {/if}

        {#each virtualRows as row, index (index)}
          <div
            class="row virtual {row.kind}"
            class:striped={stripes && striped(HEADER_ROWS + index)}
            style:top="{(HEADER_ROWS + index) * rowHeight}px"
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
            class:striped={stripes && striped(item.listRow)}
            class:selected={selection.oid === item.entry.commit.oid || comparedFrom === item.entry.commit.oid}
            class:over={over === item.entry.commit.oid}
            style:top="{item.listRow * rowHeight}px"
            style:padding-left="{rowTextX(item.entry.layout.width, clipX)}px"
            role="listitem"
            oncontextmenu={(event) => {
              if (!oncontext) return;
              event.preventDefault();
              if (graph.stale) return;
              const repo = repository.current?.repo;
              if (repo !== undefined) void pick(repo, item.entry.commit.oid);
              oncontext(item.entry.commit.oid, event.clientX, event.clientY);
            }}
          >
            {#if modes.collapseMerged}
              {@const hidden = graphOverlays.foldAt(item.entry.layout.row)}
              {@const open = graphFolds.expanded.has(item.entry.commit.oid)}
              {#if hidden > 0 || open}
                <FoldToggle {open} {hidden} ontoggle={() => graphFolds.toggle(item.entry.commit.oid)} />
              {/if}
            {/if}
            {#each refs.shown as label (refLabelKey(label))}
              <RefCapsule
                {label}
                onmenu={onrefcontext &&
                  ((event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    if (graph.stale) return;
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
            {#each cells as cell (cell)}
              {#if cell === "author"}
                <span class="author truncate">{item.entry.commit.authorName}</span>
              {:else if cell === "avatar"}
                <Avatar name={item.entry.commit.authorName} email={item.entry.commit.authorEmail} />
              {:else if cell === "time"}
                <span
                  class="date time tabular truncate"
                  title={dateTooltip(item.entry.commit.timestamp, item.entry.commit.tzOffsetMinutes)}
                  >{graphTime(
                    item.entry.commit.timestamp,
                    item.entry.commit.tzOffsetMinutes,
                    Date.now() / 1000,
                    timeFormat,
                  )}</span
                >
              {:else if cell === "overlap"}
                {@const row = overlap.rows.get(item.entry.commit.oid)}
                <span
                  class="overlap {row?.overlap ?? 'none'}"
                  class:base={row?.isBase}
                  title={row ? overlapTooltip(row.shared, row.sharedTotal) : ""}
                >
                  {row?.isBase ? "base" : row ? overlapLabel(row.overlap) : ""}
                </span>
              {:else}
                <span class="oid mono tabular">{shortOid(item.entry.commit.oid)}</span>
              {/if}
            {/each}
            {#each linkStubs(item.entry.layout) as stub (stub.segment)}
              {#if stub.box.left + stub.box.size <= clipX}
                <button
                  type="button"
                  class="link-stub"
                  tabindex="-1"
                  style:left="{stub.box.left}px"
                  style:top="{stub.box.top}px"
                  style:width="{stub.box.size}px"
                  style:height="{stub.box.size}px"
                  aria-label="Go to the other end of this link"
                  title={linkTitle(stub.oids, describeEnd)}
                  onpointerenter={() => prefetch(stub.oids)}
                  onclick={(event) => {
                    event.stopPropagation();
                    jump(stub.oids[0]);
                  }}
                ></button>
              {/if}
            {/each}
          </div>
        {/each}
      </div>
      <!-- The Working Tree row stays above an empty list: it is always the first (doc/05 §3.4). -->
      {#if commitCount === 0}
        <div class="empty" style:top="{headerRows * rowHeight}px">
          <EmptyState
            title={empty.title}
            hint={empty.hint}
            action={empty.clears ? "Clear Filter" : undefined}
            onaction={onclearfilter}
          />
        </div>
      {/if}
    </div>
    <div
      class="spacer"
      style:height="{Math.max(listRows * rowHeight - viewportHeight, 0)}px"
    ></div>
  </div>
{/if}

<style>
  .scroll {
    position: relative;
    height: 100%;
    overflow: auto;
    /* The sticky viewport is as tall as the panel, so resizing moved everything after it and
       the browser scrolled to follow; the component keeps the top row itself (#13). */
    overflow-anchor: none;
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
    font-size: var(--fs-dense);
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

  /* Gives way after the branch labels, which shrink first (#12, R-331); the right columns
     never do, and past its room the graph area is cut instead. */
  .summary {
    flex: 1 1 auto;
    min-width: 0;
  }

  .author {
    flex: 0 0 auto;
    max-width: var(--author-max);
    color: var(--text-secondary);
  }

  .date,
  .overlap {
    flex: 0 0 var(--overlap-w);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-align: right;
  }

  /* Beside the avatar, one row gap from it (#14): right-aligned in its fixed column, a short
     date sat a whole column away from the face it belongs to. */
  .date.time {
    flex-basis: var(--time-w);
    text-align: left;
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

  .avatar-cell {
    display: flex;
    flex: 0 0 auto;
  }

  .oid {
    flex: 0 0 var(--hash-w);
    overflow: hidden;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .empty {
    position: absolute;
    left: 0;
    right: 0;
    z-index: 1;
  }

  /* Over the arrow of a cut link, under the canvas that draws it (R-330). */
  .link-stub {
    position: absolute;
    padding: 0;
    background: none;
    border: 0;
    cursor: pointer;
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
