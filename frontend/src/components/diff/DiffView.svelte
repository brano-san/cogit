<script lang="ts">
  import { keyLetter } from "$lib/key-letter";
  import { modals } from "$lib/modal-stack";
  import { untrack } from "svelte";
  import {
    cellKey,
    connectors,
    pairPicked,
    type ConnectorRow,
    type SearchRow,
    type SideCell,
  } from "$lib/diff-rows";
  import {
    blockKeys,
    changeAt,
    changeStarts,
    foldDiff,
    highlightedRows,
    navState,
    revealRange,
    splitRows,
    type Gap,
    type LineRange,
  } from "$lib/diff-fold";
  import { DiffSearch } from "$lib/diff-search.svelte";
  import { BAND_WIDTH, bandLeft, ribbonPath, ribbonsNear } from "$lib/diff-band";
  import DiffFindBar from "./DiffFindBar.svelte";
  import SidewaysScrollbar from "$components/common/SidewaysScrollbar.svelte";
  import {
    NO_NEWLINE_COLUMNS,
    REVEAL_MARGIN_COLUMNS,
    TRAILING_COLUMNS,
    clampOffset,
    maxOffset,
    revealOffset,
    textColumns,
    wheelSideways,
  } from "$lib/code-scroll";
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import { eolChangeText, eolLabel, layoutTip, modeChangeText } from "$lib/diff-toolbar";
  import { MAX_HIGHLIGHT_LINES, highlightLines, mergePieces, type Token } from "$lib/highlight";
  import { lineKey, toggleLine } from "$lib/selection";
  import { keepSelection } from "$lib/diff-selection";
  import { investigateTarget, openInvestigate } from "$lib/investigate/open";
  import { visibleRange } from "$lib/graph-geometry";
  import type { DiffRow, FileDiff, Hunk } from "$lib/ipc";
  // The panel above belongs to `master` and cannot grow props for the branch's own view
  // preferences, so the view reads them from the branch's own store.
  import { diff as diffStore } from "$stores/diff.svelte";

  interface Props {
    diff: FileDiff;
    path: string;
    /** Only the working tree can be staged; a commit's diff is read-only. */
    stageable?: boolean;
    onstage?: (selected: ReadonlySet<string>, reverse: boolean) => void;
    onblame?: () => void;
    whitespace?: import("$lib/ipc").Whitespace;
    onwhitespace?: (mode: import("$lib/ipc").Whitespace) => void;
    /** Show more of the file around the hunks; `whole` opens all of it (T7.3). */
    onexpand?: (whole: boolean) => void;
  }

  let {
    diff,
    path,
    stageable = false,
    onstage,
    onblame,
    whitespace = "none",
    onwhitespace,
    onexpand,
  }: Props = $props();

  const WHITESPACE_LABEL = { none: "Whitespace", trailing: "Trailing ws", all: "Ignore ws" };
  const WHITESPACE_NEXT = { none: "trailing", trailing: "all", all: "none" } as const;

  let selected = $state<Set<string>>(new Set());

  const ROW_HEIGHT = 18;
  const BUFFER_ROWS = 12;
  /** Rows of context a jump leaves above the change it lands on. */
  const LEAD = 3;

  const mode = $derived(diffStore.layout);
  let findBar: ReturnType<typeof DiffFindBar> | undefined = $state();
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  /** The change the arrows and F6 step from; `-1` above the first one (#13). */
  let current = $state(-1);
  /** Set while a jump's own scroll is in flight, so it does not re-derive `current`. */
  let jumping = false;
  /** Lines the user opened out of the folds, by old line number (#16). */
  let revealed = $state<LineRange[]>([]);
  /** The row under the pointer, which carries its block's Stage, Unstage and Discard. */
  let hoverRow = $state<number | null>(null);

  const hunks = $derived<Hunk[]>(diff.kind === "text" ? diff.hunks : []);
  const language = $derived(diff.kind === "text" ? diff.language : null);

  /** Parsed once per diff, per side: a block comment must survive the line it opened on. */
  const tokens = $derived.by(() => {
    const oldLines: string[] = [];
    const newLines: string[] = [];
    const oldAt = new Map<string, number>();
    const newAt = new Map<string, number>();

    for (const row of highlightedRows(hunks, () => unified, MAX_HIGHLIGHT_LINES)) {
      if (row.kind === "context") {
        oldAt.set("c" + row.old, oldLines.push(row.text) - 1);
        newAt.set("c" + row.new, newLines.push(row.text) - 1);
      } else if (row.kind === "delete") {
        oldAt.set("d" + row.old, oldLines.push(row.text) - 1);
      } else if (row.kind === "insert") {
        newAt.set("i" + row.new, newLines.push(row.text) - 1);
      }
    }
    return {
      old: highlightLines(oldLines, language),
      new: highlightLines(newLines, language),
      oldAt,
      newAt,
    };
  });

  function tokensFor(row: DiffRow): Token[] {
    if (row.kind === "delete") return tokens.old[tokens.oldAt.get("d" + row.old) ?? -1] ?? [];
    if (row.kind === "insert") return tokens.new[tokens.newAt.get("i" + row.new) ?? -1] ?? [];
    if (row.kind === "context") return tokens.old[tokens.oldAt.get("c" + row.old) ?? -1] ?? [];
    return [];
  }

  const unified = $derived(
    diff.kind === "text"
      ? foldDiff({
          hunks,
          oldTotal: diff.oldTotal,
          newTotal: diff.newTotal,
          context: diffStore.foldContext,
          revealed,
        })
      : [],
  );
  const split = $derived(splitRows(unified));

  const total = $derived(mode === "unified" ? unified.length : split.length);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, ROW_HEIGHT, total, BUFFER_ROWS),
  );

  const starts = $derived(
    changeStarts(
      mode === "unified"
        ? unified.map((entry) => entry.kind === "row" && lineKey(entry.row) !== null)
        : split.map(
            (entry) =>
              entry.kind === "pair" &&
              (entry.pair.left?.kind === "delete" || entry.pair.right?.kind === "insert"),
          ),
    ),
  );
  const nav = $derived(navState(starts.length, current));

  let rowsWidth = $state(0);

  const left = $derived(bandLeft(rowsWidth));

  /** Computed once per diff. Scrolling only filters it — walking every row on each frame
      would cost the 60 FPS the product promises. */
  const allRibbons = $derived.by(() => {
    if (mode !== "split") return [];
    const rows: ConnectorRow[] = split.map((entry) => (entry.kind === "pair" ? entry.pair : null));
    return connectors(rows);
  });

  const ribbons = $derived(
    ribbonsNear(allRibbons, range.start - BUFFER_ROWS, range.end + BUFFER_ROWS),
  );

  /** Only code is searchable: a hit on a fold would scroll to nothing useful. */
  const searchTexts = $derived.by<SearchRow[]>(() => {
    if (mode === "unified") {
      return unified.map((entry) => {
        if (entry.kind !== "row") return [null, null];
        return [entry.row.text, null];
      });
    }
    return split.map((entry) =>
      entry.kind === "pair" ? [entry.pair.left?.text ?? null, entry.pair.right?.text ?? null] : [null, null],
    );
  });

  const find = new DiffSearch(() => searchTexts);

  /** Twenty is not enough: `offsetWidth` is whole pixels, and the error adds up per character. */
  const PROBE = "0".repeat(100);
  /** Measured in the hidden ruler row, which has the layout of every other row. */
  let codeWidth = $state(0);
  let probeWidth = $state(0);
  const charWidth = $derived(probeWidth / PROBE.length);
  /** Pixels the code of every column is moved left by; the numbers stay put. */
  let sideways = $state(0);

  const widest = $derived.by(() => {
    let most = 0;
    for (const entry of unified) {
      if (entry.kind !== "row") continue;
      const row = entry.row;
      most = Math.max(most, textColumns(row.text) + (row.noNewline ? NO_NEWLINE_COLUMNS : 0));
    }
    return most + TRAILING_COLUMNS;
  });
  const sidewaysMax = $derived(maxOffset(widest, charWidth, codeWidth));
  const shift = $derived(clampOffset(sideways, sidewaysMax));

  function scrollToRow(index: number) {
    if (!scroller) return;
    scroller.scrollTop = Math.max(index * ROW_HEIGHT - Math.floor(viewportHeight / 2), 0);
  }

  /** Down to the hit the counter points at, and sideways when it is past an edge. */
  function revealHit() {
    const hit = find.current;
    if (!hit) return;
    scrollToRow(hit.index);
    const text = searchTexts[hit.index]?.[hit.side === "left" ? 0 : 1];
    if (text === null || text === undefined || charWidth === 0) return;
    const from = textColumns(text.slice(0, hit.from)) * charWidth;
    const to = textColumns(text.slice(0, hit.to)) * charWidth;
    sideways = revealOffset(shift, codeWidth, from, to, sidewaysMax, REVEAL_MARGIN_COLUMNS * charWidth);
  }

  function onwheel(event: WheelEvent) {
    const delta = wheelSideways(event, ROW_HEIGHT);
    if (delta === 0 || sidewaysMax === 0) return;
    event.preventDefault();
    sideways = clampOffset(shift + delta, sidewaysMax);
  }

  function openFind() {
    find.open();
    queueMicrotask(() => findBar?.focus());
  }

  /** The block a fold row opens onto, for the Stage, Unstage and Discard it carries. */
  function blockBelow(index: number): number | null {
    const next = mode === "unified" ? unified[index + 1] : split[index + 1];
    return next && next.kind !== "gap" ? next.block : null;
  }

  /** Ctrl+click opens every fold at once; a fold whose lines are not here asks for them. */
  function openGap(gap: Gap, how: "up" | "down" | "all", event: MouseEvent) {
    const whole = event.ctrlKey || event.metaKey;
    revealed = [...revealed, whole ? { from: 1, to: Number.MAX_SAFE_INTEGER } : revealRange(gap, how)];
    if (gap.loaded) return;
    if (onexpand) onexpand(true);
    else if (diffStore.repo !== null) void diffStore.expand(diffStore.repo, true);
  }

  function sign(cell: SideCell | null): string {
    if (!cell) return "";
    return cell.kind === "delete" ? "−" : cell.kind === "insert" ? "+" : "";
  }

  /** Selecting works on any diff: a commit cannot be staged, but it can be investigated. */
  function pick(row: DiffRow) {
    const key = lineKey(row);
    if (key) selected = toggleLine(selected, key);
  }

  function pickCell(cell: SideCell | null) {
    const key = cellKey(cell);
    if (key) selected = toggleLine(selected, key);
  }

  function pickBlock(block: number) {
    const keys = blockKeys(unified, block);
    const all = [...keys].every((key) => selected.has(key));
    const next = new Set(selected);
    for (const key of keys) {
      if (all) next.delete(key);
      else next.add(key);
    }
    selected = next;
  }

  function apply(reverse: boolean) {
    if (selected.size === 0) return;
    onstage?.(selected, reverse);
    selected = new Set();
  }

  /** Stage, Unstage and Discard act on one block without disturbing the line selection. */
  function applyBlock(block: number, reverse: boolean) {
    onstage?.(blockKeys(unified, block), reverse);
  }

  /** Opens the Investigate window (#15), on the selected line when there is one. */
  function startInvestigate() {
    const repo = diffStore.repo;
    const spec = diffStore.spec;
    if (repo === null || !spec) return;
    const target = investigateTarget(spec, selected);
    void openInvestigate(repo, path, target.rev, target.line);
  }

  /** Which lines a Discard is about to throw away; `null` while nothing is pending. */
  let pendingDiscard = $state<{ keys: Set<string>; label: string; of: FileDiff } | null>(null);
  let discardError = $state<string | null>(null);

  function askDiscard(keys: Set<string>, label: string) {
    if (keys.size === 0) return;
    discardError = null;
    pendingDiscard = { keys, label, of: diff };
  }

  async function confirmDiscard() {
    const pending = pendingDiscard;
    if (!pending) return;
    pendingDiscard = null;
    try {
      await diffStore.discardLines(pending.keys, pending.of);
      selected = new Set();
    } catch (err) {
      discardError = err instanceof Error ? err.message : String(err);
    }
  }

  function cells(cell: SideCell | null, index: number, side: "left" | "right") {
    if (!cell) return [];
    // A context cell on the right carries its new-side number; the old-side lookup would
    // colour it with another line's tokens.
    if (cell.kind === "context" && side === "right") {
      const own = tokens.new[tokens.newAt.get("c" + cell.line) ?? -1] ?? [];
      return mergePieces(cell.text, own, cell.inline, find.spansFor(index, side));
    }
    const row =
      cell.kind === "delete"
        ? ({ kind: "delete", old: cell.line, text: cell.text, inline: cell.inline } as const)
        : cell.kind === "insert"
          ? ({ kind: "insert", new: cell.line, text: cell.text, inline: cell.inline } as const)
          : ({ kind: "context", old: cell.line, new: cell.line, text: cell.text } as const);
    return mergePieces(cell.text, tokensFor(row), cell.inline, find.spansFor(index, side));
  }

  function settle() {
    const top = Math.floor(scrollTop / ROW_HEIGHT);
    const bottom = Math.floor((scrollTop + viewportHeight) / ROW_HEIGHT) - 1;
    current = changeAt(starts, top, bottom, LEAD);
  }

  function jump(delta: number) {
    const target = current + delta;
    if (!scroller || target < 0 || target >= starts.length) return;
    const before = scroller.scrollTop;
    scroller.scrollTop = Math.max((starts[target] ?? 0) - LEAD, 0) * ROW_HEIGHT;
    jumping = scroller.scrollTop !== before;
    current = target;
  }

  function onscroll() {
    if (!scroller) return;
    scrollTop = scroller.scrollTop;
    if (jumping) jumping = false;
    else settle();
  }

  function onkeydown(event: KeyboardEvent) {
    // A dialog above the panel has the keys (11 §1).
    if (modals.any) return;
    const ctrl = event.ctrlKey || event.metaKey;
    const key = keyLetter(event);

    if (event.key === "F6") {
      event.preventDefault();
      jump(event.shiftKey ? -1 : 1);
    } else if (ctrl && event.shiftKey && key === "d") {
      event.preventDefault();
      void diffStore.setLayout(mode === "split" ? "unified" : "split");
    } else if (ctrl && !event.shiftKey && key === "f") {
      event.preventDefault();
      openFind();
    } else if (ctrl && event.altKey && event.shiftKey && key === "l") {
      event.preventDefault();
      startInvestigate();
    } else if (find.showing && event.key === "Escape") {
      event.preventDefault();
      find.close();
    }
  }

  $effect(() => {
    void diffStore.loadPreferences();
  });

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) viewportHeight = entry.contentRect.height;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  // Lines are chosen by number in one diff: another file, or the same file on the other
  // side of the index, makes them mean other lines.
  $effect(() => {
    void path;
    void diffStore.spec?.kind;
    find.rewind();
    selected = new Set();
    revealed = [];
    pendingDiscard = null;
    discardError = null;
    sideways = 0;
    if (scroller) scroller.scrollTop = 0;
  });

  /** The hunks `selected` was chosen in. Staging a block re-diffs the file under the
      selection; only the lines that still mean the same line stay selected. */
  let selectedIn: readonly Hunk[] = [];
  $effect(() => {
    const now = hunks;
    untrack(() => {
      if (now === selectedIn) return;
      selected = keepSelection(selected, selectedIn, now);
      selectedIn = now;
    });
  });

  /** A new diff, a new layout or a resized view: the arrows follow what is on screen. */
  $effect(() => {
    void starts;
    void viewportHeight;
    untrack(settle);
  });

  /** Typing lands on the first hit. `untrack` keeps a resize from re-scrolling the view. */
  $effect(() => {
    void find.applied;
    untrack(() => {
      find.rewind();
      revealHit();
    });
  });
</script>

<svelte:window {onkeydown} />

{#snippet eof(open: boolean | undefined)}
  {#if open}<span class="eof" title="No newline at end of file">\ no newline</span>{/if}
{/snippet}

{#snippet cellGutter(cell: SideCell | null)}
  {@const key = cellKey(cell)}
  <span
    class="gutter"
    class:picked={key !== null && selected.has(key)}
    role="button"
    tabindex="-1"
    onclick={() => pickCell(cell)}
    onkeydown={(e) => e.key === "Enter" && pickCell(cell)}
    >{stageable && key !== null ? (selected.has(key) ? "■" : "□") : ""}</span
  >
{/snippet}

{#snippet blockActions(block: number)}
  {#if stageable}
    <span class="acts">
      <button type="button" title="Select every changed line of this block" onclick={() => pickBlock(block)}
        >Select</button
      >
      <button
        type="button"
        title="Stage this block"
        disabled={!diffStore.lineActions.stage}
        onclick={() => applyBlock(block, false)}>Stage</button
      >
      <button
        type="button"
        title="Unstage this block"
        disabled={!diffStore.lineActions.unstage}
        onclick={() => applyBlock(block, true)}>Unstage</button
      >
      <button
        type="button"
        class="danger"
        title="Throw this block away (always asks first)"
        disabled={!diffStore.lineActions.discard}
        onclick={() => askDiscard(blockKeys(unified, block), "this block")}>Discard</button
      >
    </span>
  {/if}
{/snippet}

{#snippet fold(gap: Gap, index: number)}
  {@const below = blockBelow(index)}
  <div class="fold" role="group" aria-label="{gap.hidden} lines hidden">
    {#if gap.up}
      <button
        type="button"
        class="arrow"
        title="Show 20 more lines above the change below (Ctrl+click: every hidden line)"
        onclick={(event) => openGap(gap, "up", event)}>▲</button
      >
    {/if}
    <button
      type="button"
      class="count"
      title="Show all {gap.hidden} hidden lines (Ctrl+click: every hidden line in the file)"
      onclick={(event) => openGap(gap, "all", event)}>{gap.hidden} lines hidden · show</button
    >
    {#if gap.down}
      <button
        type="button"
        class="arrow"
        title="Show 20 more lines below the change above (Ctrl+click: every hidden line)"
        onclick={(event) => openGap(gap, "down", event)}>▼</button
      >
    {/if}
    {#if gap.context}<span class="where truncate">{gap.context}</span>{/if}
    {#if below !== null}{@render blockActions(below)}{/if}
  </div>
{/snippet}

<div class="diff">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    {#if diff.kind === "text"}
      {@const eol = eolLabel(diff.eol, diff.oldTotal, diff.newTotal)}
      <span class="eol" title={eol.title}>{eol.text}</span>
      {#if diff.lossyEncoding}<span class="warn">not valid UTF-8</span>{/if}
      <button type="button" disabled={!nav.prev} onclick={() => jump(-1)} title="Previous change (Shift+F6)"
        >▲</button
      >
      <button type="button" disabled={!nav.next} onclick={() => jump(1)} title="Next change (F6)">▼</button>
      <button
        type="button"
        class:active={find.showing}
        title="Search the lines shown in this diff (Ctrl+F); open the folds to search the whole file"
        onclick={() => (find.showing ? find.close() : openFind())}>Find</button
      >
      {#if stageable}
        <span class="picked tabular">{selected.size ? `${selected.size} selected` : ""}</span>
        <button
          type="button"
          disabled={selected.size === 0 || !diffStore.lineActions.stage}
          onclick={() => apply(false)}>Stage lines</button
        >
        <button
          type="button"
          disabled={selected.size === 0 || !diffStore.lineActions.unstage}
          onclick={() => apply(true)}>Unstage lines</button
        >
        <button
          type="button"
          class="danger"
          disabled={selected.size === 0 || !diffStore.lineActions.discard}
          title="Throw the selected lines away (always asks first)"
          onclick={() => askDiscard(new Set(selected), `${selected.size} selected lines`)}
          >Discard lines</button
        >
      {/if}
    {/if}
    <!-- A file the mode hides entirely still needs the button that shows it (F-067). -->
    {#if onwhitespace && (diff.kind === "text" || diff.kind === "whitespaceOnly")}
      <button
        type="button"
        class:active={whitespace !== "none"}
        title="Off → trailing → all"
        onclick={() => onwhitespace(WHITESPACE_NEXT[whitespace])}
        >{WHITESPACE_LABEL[whitespace]}</button
      >
    {/if}
    {#if diff.kind === "text"}
      {#if onblame}
        <button type="button" title="Annotate every line with its commit" onclick={() => onblame()}
          >Blame</button
        >
      {/if}
      <button
        type="button"
        disabled={diffStore.repo === null}
        title="Trace where the lines came from, starting at the selected one (Ctrl+Alt+Shift+L)"
        onclick={startInvestigate}>Investigate</button
      >
      <button
        type="button"
        class:active={!diffStore.showMoves}
        title="Show a moved block as an ordinary deletion plus addition"
        onclick={() => diffStore.setShowMoves(!diffStore.showMoves)}
      >
        {diffStore.showMoves ? "Moves" : "No moves"}
      </button>
      <button
        type="button"
        class="mode"
        title={layoutTip(mode)}
        onclick={() => diffStore.setLayout(mode === "split" ? "unified" : "split")}
      >
        {mode === "split" ? "Unified" : "Side by side"}
      </button>
    {/if}
  </div>

  {#if pendingDiscard}
    <ConfirmDialog
      title="Discard lines"
      message="Throw away {pendingDiscard.label} in {path}? Undo can put them back."
      confirm="Discard"
      warning
      onanswer={(yes) => (yes ? void confirmDiscard() : (pendingDiscard = null))}
    />
  {/if}
  {#if discardError}
    <div class="confirm">
      <span class="grow warn">{discardError}</span>
      <button type="button" onclick={() => (discardError = null)}>Dismiss</button>
    </div>
  {/if}

  {#if find.showing && diff.kind === "text"}
    <DiffFindBar bind:this={findBar} {find} reveal={revealHit} />
  {/if}

  {#if diff.kind === "unchanged"}
    <p class="message">No change in this file.</p>
  {:else if diff.kind === "modeOnly"}
    <p class="message">
      Only the file mode changed: {modeChangeText(diff.oldMode, diff.newMode)}. The content is
      identical.
    </p>
  {:else if diff.kind === "emptyFile"}
    <p class="message">
      {diff.added ? "An empty file was added" : "An empty file was deleted"}: there are no lines
      to show.
    </p>
  {:else if diff.kind === "whitespaceOnly"}
    <p class="message warn">
      Only whitespace changed, and {WHITESPACE_LABEL[whitespace]} hides it.
      {onwhitespace
        ? `Click ${WHITESPACE_LABEL[whitespace]} in the bar above until it reads ${WHITESPACE_LABEL.none} to see the diff.`
        : "Choose Show every change in Preferences ▸ Diff View ▸ Whitespace to see the diff."}
    </p>
  {:else if diff.kind === "eolOnly"}
    <p class="message">
      Only the line endings changed: {eolChangeText(diff.from, diff.to)}. The content is identical.
    </p>
  {:else if diff.kind === "binary"}
    <p class="message">Binary file — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "image"}
    <p class="message">Image ({diff.mime}) — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "tooLarge"}
    <p class="message">File is too large to diff ({diff.size} bytes).</p>
  {:else if diff.kind === "folder"}
    <p class="message">
      {diff.repository
        ? "A repository nested inside this one, not a submodule: Git tracks none of its files. Add it as a submodule or ignore it."
        : "An untracked folder: Git tracks none of its files yet. Stage it to add them all, or ignore it."}
    </p>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="scroll" bind:this={scroller} {onscroll} {onwheel} onmouseleave={() => (hoverRow = null)}>
      <div
        class="rows"
        style:height="{total * ROW_HEIGHT}px"
        style:--shift="{shift}px"
        bind:clientWidth={rowsWidth}
      >
        <div class="line ruler" aria-hidden="true">
          <span class="gutter"></span>
          <span class="num"></span>
          {#if mode === "unified"}<span class="num"></span>{/if}
          <span class="sign"></span>
          <span class="code mono" class:side={mode === "split"} bind:clientWidth={codeWidth}
            ><span class="probe" bind:offsetWidth={probeWidth}>{PROBE}</span></span
          >
          {#if mode === "split"}
            <span class="gap"></span>
            <span class="gutter"></span>
            <span class="num"></span>
            <span class="sign"></span>
            <span class="code mono side"></span>
          {/if}
        </div>
        {#if mode === "split" && ribbons.length > 0}
          <svg
            class="band"
            style:left="{left}px"
            width={BAND_WIDTH}
            height={total * ROW_HEIGHT}
            aria-hidden="true"
          >
            {#each ribbons as ribbon, i (i)}
              <path class="ribbon" class:moved={ribbon.moved} d={ribbonPath(ribbon, ROW_HEIGHT)} />
            {/each}
          </svg>
        {/if}
        {#if mode === "unified"}
          {#each unified.slice(range.start, range.end) as entry, index (range.start + index)}
            {@const rowIndex = range.start + index}
            {#if entry.kind === "gap"}
              <div class="line" style:top="{rowIndex * ROW_HEIGHT}px">
                {@render fold(entry.gap, rowIndex)}
              </div>
            {:else}
              {@const key = lineKey(entry.row)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="line"
                class:staging={stageable && key !== null && selected.has(key)}
                class:marked={!stageable && key !== null && selected.has(key)}
                style:top="{rowIndex * ROW_HEIGHT}px"
                onmouseenter={() => (hoverRow = rowIndex)}
              >
                {#if entry.row.kind === "context"}
                  <span class="gutter"></span>
                  <span class="num">{entry.row.old}</span>
                  <span class="num">{entry.row.new}</span>
                  <span class="sign"></span>
                  <span class="code mono"
                    ><span class="text"
                      >{#each mergePieces(entry.row.text, tokensFor(entry.row), [], find.spansFor(rowIndex, "left")) as piece, i (i)}<span
                          class={piece.cls}
                          class:hit={piece.hit}
                          class:current={find.isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                        >{/each}{@render eof(entry.row.noNewline)}</span
                    ></span
                  >
                {:else if entry.row.kind === "delete"}
                  {@const row = entry.row}
                  <span
                    class="gutter"
                    class:picked={selected.has("d:" + row.old)}
                    role="button"
                    tabindex="-1"
                    onclick={() => pick(row)}
                    onkeydown={(e) => e.key === "Enter" && pick(row)}
                    >{stageable ? (selected.has("d:" + row.old) ? "■" : "□") : ""}</span
                  >
                  <span class="num">{row.old}</span>
                  <span class="num"></span>
                  <span class="sign del" class:moved={row.moved}>−</span>
                  <span class="code mono del" class:moved={row.moved}
                    ><span class="text"
                      >{#each mergePieces(row.text, tokensFor(row), row.inline, find.spansFor(rowIndex, "left")) as piece, i (i)}<span
                          class="{piece.cls}"
                          class:word={piece.changed}
                          class:hit={piece.hit}
                          class:current={find.isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                        >{/each}{@render eof(row.noNewline)}</span
                    ></span
                  >
                {:else if entry.row.kind === "insert"}
                  {@const row = entry.row}
                  <span
                    class="gutter"
                    class:picked={selected.has("i:" + row.new)}
                    role="button"
                    tabindex="-1"
                    onclick={() => pick(row)}
                    onkeydown={(e) => e.key === "Enter" && pick(row)}
                    >{stageable ? (selected.has("i:" + row.new) ? "■" : "□") : ""}</span
                  >
                  <span class="num"></span>
                  <span class="num">{row.new}</span>
                  <span class="sign add" class:moved={row.moved}>+</span>
                  <span class="code mono add" class:moved={row.moved}
                    ><span class="text"
                      >{#each mergePieces(row.text, tokensFor(row), row.inline, find.spansFor(rowIndex, "left")) as piece, i (i)}<span
                          class="{piece.cls}"
                          class:word={piece.changed}
                          class:hit={piece.hit}
                          class:current={find.isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                        >{/each}{@render eof(row.noNewline)}</span
                    ></span
                  >
                {/if}
                {#if hoverRow === rowIndex}{@render blockActions(entry.block)}{/if}
              </div>
            {/if}
          {/each}
        {:else}
          {#each split.slice(range.start, range.end) as entry, index (range.start + index)}
            {@const rowIndex = range.start + index}
            {#if entry.kind === "gap"}
              <div class="line" style:top="{rowIndex * ROW_HEIGHT}px">
                {@render fold(entry.gap, rowIndex)}
              </div>
            {:else}
              {@const picked = pairPicked(entry.pair, selected)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="line"
                class:staging={stageable && picked}
                class:marked={!stageable && picked}
                style:top="{rowIndex * ROW_HEIGHT}px"
                onmouseenter={() => (hoverRow = rowIndex)}
              >
                {@render cellGutter(entry.pair.left)}
                <span class="num">{entry.pair.left?.line ?? ""}</span>
                <span
                  class="sign"
                  class:del={entry.pair.left?.kind === "delete"}
                  class:moved={entry.pair.left?.moved}>{sign(entry.pair.left)}</span
                >
                <span
                  class="code mono side"
                  class:del={entry.pair.left?.kind === "delete"}
                  class:moved={entry.pair.left?.moved}
                  ><span class="text"
                    >{#each cells(entry.pair.left, rowIndex, "left") as piece, i (i)}<span
                        class="{piece.cls}"
                        class:word={piece.changed}
                        class:hit={piece.hit}
                        class:current={find.isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                      >{/each}{@render eof(entry.pair.left?.noNewline)}</span
                  ></span
                >
                <span class="gap"></span>
                {@render cellGutter(entry.pair.right)}
                <span class="num">{entry.pair.right?.line ?? ""}</span>
                <span
                  class="sign"
                  class:add={entry.pair.right?.kind === "insert"}
                  class:moved={entry.pair.right?.moved}>{sign(entry.pair.right)}</span
                >
                <span
                  class="code mono side"
                  class:add={entry.pair.right?.kind === "insert"}
                  class:moved={entry.pair.right?.moved}
                  ><span class="text"
                    >{#each cells(entry.pair.right, rowIndex, "right") as piece, i (i)}<span
                        class="{piece.cls}"
                        class:word={piece.changed}
                        class:hit={piece.hit}
                        class:current={find.isCurrent(rowIndex, "right", piece.start)}>{piece.text}</span
                      >{/each}{@render eof(entry.pair.right?.noNewline)}</span
                  ></span
                >
                {#if hoverRow === rowIndex}{@render blockActions(entry.block)}{/if}
              </div>
            {/if}
          {/each}
        {/if}
      </div>
    </div>
    <SidewaysScrollbar offset={shift} max={sidewaysMax} onscroll={(offset) => (sideways = offset)} />
  {/if}
</div>

<style>
  .diff {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  /* Chrome, not content: the raised surface and a rule set it apart from the diff (#18). */
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    min-height: 32px;
    padding: var(--sp-2) var(--sp-4);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
  }

  .eol {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .warn {
    color: var(--status-modify);
    font-size: 11px;
  }

  .bar button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .bar button.active {
    color: var(--status-modify);
    border-color: var(--status-modify);
  }

  .bar button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  /* 06 §6: a control that cannot act says so, and does not light up under the pointer. */
  .bar button:disabled,
  .acts button:disabled {
    opacity: 0.4;
  }

  /* Sideways the code moves by `--shift`, under the scrollbar below the rows (R-470). */
  .scroll {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
  }

  .rows {
    position: relative;
  }

  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: stretch;
    height: 18px;
    line-height: 18px;
    font-size: var(--fs-code);
    white-space: pre;
  }

  .gutter {
    flex: 0 0 auto;
    width: 14px;
    color: var(--text-secondary);
    font-size: 9px;
    text-align: center;
    cursor: default;
  }

  .gutter.picked {
    color: var(--status-add);
  }

  /* What the next Stage or Unstage will act on, marked on the row and not just in the
     14-pixel gutter, so the user can see the extent of it at a glance (T6.7). */
  .line.staging {
    background: var(--c-add-soft);
    box-shadow: inset 2px 0 0 var(--status-add);
  }

  /* A read-only diff can still be selected, for Investigate; it just stages nothing. */
  .line.marked {
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .acts {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
    margin-left: auto;
    padding-right: var(--sp-3);
  }

  /* On a code row the buttons float over the end of the line instead of pushing it. */
  .line > .acts {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    padding-left: var(--sp-3);
    background: var(--surface-panel);
  }

  .acts button {
    height: 14px;
    padding: 0 var(--sp-2);
    background: var(--surface-input);
    color: var(--text-secondary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: 9px;
    line-height: 12px;
    cursor: default;
  }

  .acts button:hover:not(:disabled) {
    color: var(--text-primary);
  }

  button.danger {
    color: var(--status-delete);
  }

  button.danger:hover {
    border-color: var(--status-delete);
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    background: var(--surface-raised);
    font-size: var(--fs-dense);
  }

  .confirm button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .grow {
    flex: 1 1 auto;
  }

  .picked {
    color: var(--status-add);
    font-size: 10px;
  }

  .num {
    flex: 0 0 auto;
    width: 44px;
    padding-right: var(--sp-3);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .code {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    user-select: text;
  }

  /* One offset for every column: side by side, both halves move together. */
  .text {
    display: inline-block;
    vertical-align: top;
    transform: translateX(calc(-1 * var(--shift, 0px)));
  }

  /* The layout of a row, never seen: it measures a code column and one character. */
  .ruler {
    top: 0;
    visibility: hidden;
    pointer-events: none;
  }

  .probe {
    display: inline-block;
  }

  /* The +/− column: its own, two spaces clear of the code, never copied with it (#9). */
  .sign {
    flex: 0 0 auto;
    width: 3.5ch;
    padding-left: 0.5ch;
    box-sizing: border-box;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    user-select: none;
  }

  .sign.del {
    background: var(--c-deleted-bg);
    color: var(--status-delete);
  }

  .sign.add {
    background: var(--c-added-bg);
    color: var(--status-add);
  }

  .sign.moved {
    background: var(--c-stash-bg);
    color: var(--status-stash);
  }

  .side {
    flex: 1 1 50%;
  }

  /* Reserves the strip the ribbons are drawn over. Width must match `BAND_WIDTH`. */
  .gap {
    flex: 0 0 28px;
  }

  .band {
    position: absolute;
    top: 0;
    pointer-events: none;
  }

  .ribbon {
    fill: var(--c-added-bg);
    stroke: none;
  }

  /* A move goes somewhere else in the file, so its ribbon is an outline, not a fill. */
  .ribbon.moved {
    fill: none;
    stroke: var(--status-stash);
    stroke-width: 1.5;
    stroke-dasharray: 4 3;
  }

  .word {
    border-radius: 2px;
    background: var(--c-neutral-soft);
    font-weight: 600;
  }

  .code.del .word {
    background: color-mix(in srgb, var(--c-deleted) 45%, transparent);
    color: var(--text-primary);
  }

  .code.add .word {
    background: color-mix(in srgb, var(--c-added) 45%, transparent);
    color: var(--text-primary);
  }

  /* Every match is marked; the one the counter points at is the bright one. */
  .hit {
    border-radius: 2px;
    background: var(--c-search-hit);
  }

  .hit.current {
    background: var(--c-search-current);
    color: var(--c-search-ink);
  }

  .code.del {
    background: var(--c-deleted-bg);
    color: var(--status-delete);
  }

  .code.add {
    background: var(--c-added-bg);
    color: var(--status-add);
  }

  /* A moved block is one fact, not a deletion plus an addition (T7.9). A moved row is
     `del` or `add` as well, so this comes after them and wins at the same specificity. */
  .code.moved {
    background: var(--c-stash-bg);
    color: var(--status-stash);
  }

  .code.moved .word {
    background: color-mix(in srgb, var(--status-stash) 30%, transparent);
  }

  /* Where @@ used to be: one band across both halves, saying what is hidden (#16). */
  .fold {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 1 1 auto;
    min-width: 0;
    padding-left: var(--sp-4);
    background: var(--surface-raised);
    box-shadow:
      inset 0 1px 0 var(--divider),
      inset 0 -1px 0 var(--divider);
    color: var(--text-secondary);
    font-family: var(--font-ui);
    font-size: var(--fs-header);
  }

  .fold > button {
    flex: 0 0 auto;
    height: 14px;
    padding: 0 var(--sp-2);
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    font: inherit;
    line-height: 14px;
    cursor: default;
  }

  .fold > button:hover {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .fold .arrow {
    font-size: 9px;
  }

  .where {
    min-width: 0;
    margin-left: var(--sp-3);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    opacity: 0.8;
  }

  /* What `git diff` prints under the line; here at its end, and never copied with it. */
  .eof {
    margin-left: 1ch;
    color: var(--text-secondary);
    font-style: italic;
    user-select: none;
  }

  .message.warn {
    color: var(--status-modify);
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
