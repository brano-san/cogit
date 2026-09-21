<script lang="ts">
  import { untrack } from "svelte";
  import {
    connectors,
    flatten,
    gapBetween,
    pairRows,
    searchRows,
    stepHit,
    type ConnectorRow,
    type FlatEntry,
    type SearchRow,
    type SideCell,
  } from "$lib/diff-rows";
  import { highlightLines, mergePieces, type Token } from "$lib/highlight";
  import { hunkSelection, lineKey, toggleLine } from "$lib/selection";
  import { visibleRange } from "$lib/graph-geometry";
  import type { FileDiff, Hunk } from "$lib/ipc";
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
  /** Must match `.num` and `.band` in the stylesheet: the overlay is positioned by hand. */
  const NUM_WIDTH = 44;
  const BAND_WIDTH = 28;

  const mode = $derived(diffStore.layout);
  let finding = $state(false);
  let query = $state("");
  let hitAt = $state(0);
  let findBox: HTMLInputElement | undefined = $state();
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);
  let current = $state(0);

  const hunks = $derived<Hunk[]>(diff.kind === "text" ? diff.hunks : []);
  const language = $derived(diff.kind === "text" ? diff.language : null);

  /** Parsed once per diff, per side: a block comment must survive the line it opened on. */
  const tokens = $derived.by(() => {
    const oldLines: string[] = [];
    const newLines: string[] = [];
    const oldAt = new Map<string, number>();
    const newAt = new Map<string, number>();

    for (const hunk of hunks) {
      for (const row of hunk.rows) {
        if (row.kind === "context") {
          oldAt.set("c" + row.old, oldLines.push(row.text) - 1);
          newAt.set("c" + row.new, newLines.push(row.text) - 1);
        } else if (row.kind === "delete") {
          oldAt.set("d" + row.old, oldLines.push(row.text) - 1);
        } else if (row.kind === "insert") {
          newAt.set("i" + row.new, newLines.push(row.text) - 1);
        }
      }
    }
    return {
      old: highlightLines(oldLines, language),
      new: highlightLines(newLines, language),
      oldAt,
      newAt,
    };
  });

  function tokensFor(row: import("$lib/ipc").DiffRow): Token[] {
    if (row.kind === "delete") return tokens.old[tokens.oldAt.get("d" + row.old) ?? -1] ?? [];
    if (row.kind === "insert") return tokens.new[tokens.newAt.get("i" + row.new) ?? -1] ?? [];
    if (row.kind === "context") return tokens.old[tokens.oldAt.get("c" + row.old) ?? -1] ?? [];
    return [];
  }
  const unified = $derived(flatten(hunks));
  const split = $derived.by(() => {
    const out: { hunk: number; header?: string; pair?: ReturnType<typeof pairRows>[number] }[] = [];
    hunks.forEach((hunk, index) => {
      out.push({ hunk: index, header: hunk.header });
      for (const pair of pairRows(hunk.rows)) out.push({ hunk: index, pair });
    });
    return out;
  });

  const total = $derived(mode === "unified" ? unified.length : split.length);
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, ROW_HEIGHT, total, BUFFER_ROWS),
  );
  const headerOffsets = $derived.by(() => {
    const rows = mode === "unified" ? unified : split;
    const offsets: number[] = [];
    rows.forEach((row, index) => {
      const isHeader = mode === "unified" ? (row as FlatEntry).kind === "header" : "header" in row;
      if (isHeader) offsets.push(index);
    });
    return offsets;
  });

  let rowsWidth = $state(0);

  /** `[num][code][band][num][code]` with equal code columns puts the band dead centre.
      Every part is `border-box`, so the widths in the stylesheet are the real ones. */
  const bandLeft = $derived(Math.max((rowsWidth - BAND_WIDTH) / 2, NUM_WIDTH));

  /** Computed once per diff. Scrolling only filters it — walking every row on each frame
      would cost the 60 FPS the product promises. */
  const allRibbons = $derived.by(() => {
    if (mode !== "split") return [];
    const rows: ConnectorRow[] = split.map((entry) => entry.pair ?? null);
    return connectors(rows);
  });

  /** Only what is near the viewport is drawn; the band is as tall as the whole file. */
  const ribbons = $derived.by(() => {
    const from = range.start - BUFFER_ROWS;
    const to = range.end + BUFFER_ROWS;
    return allRibbons.filter(
      (c) => Math.max(c.fromBottom, c.toBottom) >= from && Math.min(c.fromTop, c.toTop) <= to,
    );
  });

  /** A closed ribbon: down the left edge, across on a curve, back up the right edge. */
  function ribbonPath(c: { fromTop: number; fromBottom: number; toTop: number; toBottom: number }) {
    const w = BAND_WIDTH;
    const bend = w / 2;
    const a = c.fromTop * ROW_HEIGHT;
    const b = (c.fromBottom + 1) * ROW_HEIGHT;
    const x = c.toTop * ROW_HEIGHT;
    const y = (c.toBottom + 1) * ROW_HEIGHT;
    return `M 0 ${a} C ${bend} ${a} ${bend} ${x} ${w} ${x} L ${w} ${y} C ${bend} ${y} ${bend} ${b} 0 ${b} Z`;
  }

  /** Only code is searchable: a hit on a hunk header would scroll to nothing useful. */
  const searchTexts = $derived.by<SearchRow[]>(() => {
    if (mode === "unified") {
      return unified.map((entry) => {
        if (entry.kind !== "row" || entry.row.kind === "collapsed") return [null, null];
        return [entry.row.text, null];
      });
    }
    return split.map((entry) =>
      entry.pair ? [entry.pair.left?.text ?? null, entry.pair.right?.text ?? null] : [null, null],
    );
  });

  const hits = $derived(searchRows(searchTexts, query));
  const currentHit = $derived(hits[hitAt] ?? null);

  const hitSpans = $derived.by(() => {
    const byCell = new Map<string, [number, number][]>();
    for (const hit of hits) {
      const key = `${hit.index}:${hit.side}`;
      const spans = byCell.get(key) ?? [];
      spans.push([hit.from, hit.to]);
      byCell.set(key, spans);
    }
    return byCell;
  });

  function spansFor(index: number, side: "left" | "right"): [number, number][] {
    return hitSpans.get(`${index}:${side}`) ?? [];
  }

  /** The one hit the counter is pointing at, told apart from the rest it looks like. */
  function isCurrent(index: number, side: "left" | "right", start: number): boolean {
    if (!currentHit || currentHit.index !== index || currentHit.side !== side) return false;
    return start >= currentHit.from && start < currentHit.to;
  }

  function scrollToRow(index: number) {
    if (!scroller) return;
    scroller.scrollTop = Math.max(index * ROW_HEIGHT - Math.floor(viewportHeight / 2), 0);
  }

  function goHit(delta: number) {
    if (hits.length === 0) return;
    hitAt = stepHit(hits, hitAt, delta);
    const hit = hits[hitAt];
    if (hit) scrollToRow(hit.index);
  }

  function openFind() {
    finding = true;
    queueMicrotask(() => findBox?.select());
  }

  function closeFind() {
    finding = false;
    query = "";
  }

  /** Lines the diff is not showing above each hunk, for the expander. */
  const hidden = $derived(
    hunks.map((hunk, index) => gapBetween(index === 0 ? null : (hunks[index - 1] ?? null), hunk)),
  );

  function sign(cell: SideCell | null): string {
    if (!cell) return "";
    return cell.kind === "delete" ? "−" : cell.kind === "insert" ? "+" : " ";
  }

  function pick(row: import("$lib/ipc").DiffRow) {
    const key = lineKey(row);
    if (key && stageable) selected = toggleLine(selected, key);
  }

  function pickHunk(index: number) {
    const hunk = hunks[index];
    if (!hunk || !stageable) return;
    const keys = hunkSelection(hunk);
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

  /** Stage, Unstage and Discard act on one hunk without disturbing the line selection. */
  function applyHunk(index: number, reverse: boolean) {
    const hunk = hunks[index];
    if (!hunk) return;
    onstage?.(hunkSelection(hunk), reverse);
  }

  /** Which lines a Discard is about to throw away; `null` while nothing is pending. */
  let pendingDiscard = $state<{ keys: Set<string>; label: string } | null>(null);
  let discardError = $state<string | null>(null);

  function askDiscard(keys: Set<string>, label: string) {
    if (keys.size === 0) return;
    discardError = null;
    pendingDiscard = { keys, label };
  }

  async function confirmDiscard() {
    const pending = pendingDiscard;
    if (!pending) return;
    pendingDiscard = null;
    try {
      await diffStore.discardLines(pending.keys);
      selected = new Set();
    } catch (err) {
      discardError = err instanceof Error ? err.message : String(err);
    }
  }

  function cells(cell: SideCell | null, index: number, side: "left" | "right") {
    if (!cell) return [];
    const row =
      cell.kind === "delete"
        ? ({ kind: "delete", old: cell.line, text: cell.text, inline: cell.inline } as const)
        : cell.kind === "insert"
          ? ({ kind: "insert", new: cell.line, text: cell.text, inline: cell.inline } as const)
          : ({ kind: "context", old: cell.line, new: cell.line, text: cell.text } as const);
    return mergePieces(cell.text, tokensFor(row), cell.inline, spansFor(index, side));
  }

  function jump(delta: number) {
    if (headerOffsets.length === 0 || !scroller) return;
    current = Math.min(Math.max(current + delta, 0), headerOffsets.length - 1);
    scroller.scrollTop = (headerOffsets[current] ?? 0) * ROW_HEIGHT;
  }

  function onkeydown(event: KeyboardEvent) {
    const ctrl = event.ctrlKey || event.metaKey;
    const key = event.key.toLowerCase();

    if (event.key === "F6") {
      event.preventDefault();
      jump(event.shiftKey ? -1 : 1);
    } else if (ctrl && event.shiftKey && key === "d") {
      event.preventDefault();
      void diffStore.setLayout(mode === "split" ? "unified" : "split");
    } else if (ctrl && !event.shiftKey && key === "f") {
      event.preventDefault();
      openFind();
    } else if (finding && event.key === "Escape") {
      event.preventDefault();
      closeFind();
    }
  }

  /** Enter walks the hits; the input keeps the key to itself so the page does not scroll. */
  function onfindkey(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      goHit(event.shiftKey ? -1 : 1);
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

  $effect(() => {
    void path;
    current = 0;
    hitAt = 0;
    selected = new Set();
    if (scroller) scroller.scrollTop = 0;
  });

  /** Typing lands on the first hit. `untrack` keeps a resize from re-scrolling the view. */
  $effect(() => {
    void query;
    untrack(() => {
      hitAt = 0;
      const first = hits[0];
      if (first) scrollToRow(first.index);
    });
  });
</script>

<svelte:window {onkeydown} />

{#snippet hunkActions(index: number)}
  {#if stageable}
    <span class="acts">
      <button type="button" title="Stage this hunk" onclick={() => applyHunk(index, false)}
        >Stage</button
      >
      <button type="button" title="Unstage this hunk" onclick={() => applyHunk(index, true)}
        >Unstage</button
      >
      <button
        type="button"
        class="danger"
        title="Throw this hunk away (always asks first)"
        onclick={() => askDiscard(hunkSelection(hunks[index]!), `hunk ${index + 1}`)}
        >Discard</button
      >
    </span>
  {/if}
{/snippet}

<div class="diff">
  <div class="bar">
    <span class="path mono truncate">{path}</span>
    {#if diff.kind === "text"}
      <span class="eol">{diff.eol.old} → {diff.eol.new}</span>
      {#if diff.lossyEncoding}<span class="warn">not valid UTF-8</span>{/if}
      <button type="button" onclick={() => jump(-1)} title="Previous change (Shift+F6)">▲</button>
      <button type="button" onclick={() => jump(1)} title="Next change (F6)">▼</button>
      <button
        type="button"
        class:active={finding}
        title="Search inside this diff (Ctrl+F)"
        onclick={() => (finding ? closeFind() : openFind())}>Find</button
      >
      {#if stageable}
        <span class="picked tabular">{selected.size ? `${selected.size} selected` : ""}</span>
        <button type="button" disabled={selected.size === 0} onclick={() => apply(false)}
          >Stage lines</button
        >
        <button type="button" disabled={selected.size === 0} onclick={() => apply(true)}
          >Unstage lines</button
        >
        <button
          type="button"
          class="danger"
          disabled={selected.size === 0}
          title="Throw the selected lines away (always asks first)"
          onclick={() => askDiscard(new Set(selected), `${selected.size} selected lines`)}
          >Discard lines</button
        >
      {/if}
      {#if onwhitespace}
        <button
          type="button"
          class:active={whitespace !== "none"}
          title="Off → trailing → all"
          onclick={() => onwhitespace(WHITESPACE_NEXT[whitespace])}
          >{WHITESPACE_LABEL[whitespace]}</button
        >
      {/if}
      {#if onblame}
        <button type="button" title="Annotate every line with its commit" onclick={() => onblame()}
          >Blame</button
        >
      {/if}
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
        title="Remembered between runs"
        onclick={() => diffStore.setLayout(mode === "split" ? "unified" : "split")}
      >
        {mode === "split" ? "Unified" : "Side by side"}
      </button>
    {/if}
  </div>

  {#if pendingDiscard}
    <div class="confirm" role="alertdialog" aria-label="Confirm discard">
      <span class="grow">Throw away {pendingDiscard.label}? This cannot be undone.</span>
      <button type="button" onclick={() => (pendingDiscard = null)}>Cancel</button>
      <button type="button" class="danger" onclick={confirmDiscard}>Discard</button>
    </div>
  {:else if discardError}
    <div class="confirm">
      <span class="grow warn">{discardError}</span>
      <button type="button" onclick={() => (discardError = null)}>Dismiss</button>
    </div>
  {/if}

  {#if finding && diff.kind === "text"}
    <div class="find">
      <input
        bind:this={findBox}
        bind:value={query}
        type="search"
        placeholder="Find in diff"
        spellcheck="false"
        aria-label="Find in diff"
        onkeydown={onfindkey}
      />
      <span class="count tabular">
        {#if query.trim() === ""}
          &nbsp;
        {:else if hits.length === 0}
          no matches
        {:else}
          {hitAt + 1} / {hits.length}
        {/if}
      </span>
      <button
        type="button"
        disabled={hits.length === 0}
        title="Previous match (Shift+Enter)"
        onclick={() => goHit(-1)}>▲</button
      >
      <button
        type="button"
        disabled={hits.length === 0}
        title="Next match (Enter)"
        onclick={() => goHit(1)}>▼</button
      >
      <button type="button" title="Close (Escape)" onclick={closeFind}>✕</button>
    </div>
  {/if}

  {#if diff.kind === "unchanged"}
    <p class="message">No change in this file.</p>
  {:else if diff.kind === "whitespaceOnly"}
    <p class="message warn">
      Only whitespace changed. The current mode hides it — switch the whitespace button off
      to see the diff.
    </p>
  {:else if diff.kind === "eolOnly"}
    <p class="message">
      Only the line endings changed: {diff.from} → {diff.to}. The content is identical.
    </p>
  {:else if diff.kind === "binary"}
    <p class="message">Binary file — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "image"}
    <p class="message">Image ({diff.mime}) — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "tooLarge"}
    <p class="message">File is too large to diff ({diff.size} bytes).</p>
  {:else}
    <div class="scroll" bind:this={scroller} onscroll={() => scroller && (scrollTop = scroller.scrollTop)}>
      <div class="rows" style:height="{total * ROW_HEIGHT}px" bind:clientWidth={rowsWidth}>
        {#if mode === "split" && ribbons.length > 0}
          <svg
            class="band"
            style:left="{bandLeft}px"
            width={BAND_WIDTH}
            height={total * ROW_HEIGHT}
            aria-hidden="true"
          >
            {#each ribbons as ribbon, i (i)}
              <path class="ribbon" class:moved={ribbon.moved} d={ribbonPath(ribbon)} />
            {/each}
          </svg>
        {/if}
        {#if mode === "unified"}
          {#each unified.slice(range.start, range.end) as entry, index (range.start + index)}
            {@const rowIndex = range.start + index}
            {@const key = entry.kind === "row" ? lineKey(entry.row) : null}
            <div
              class="line"
              class:staging={key !== null && selected.has(key)}
              style:top="{rowIndex * ROW_HEIGHT}px"
            >
              {#if entry.kind === "header"}
                <span
                  class="header mono"
                  class:clickable={stageable}
                  role="button"
                  tabindex="-1"
                  onclick={() => pickHunk(entry.hunk)}
                  onkeydown={(e) => e.key === "Enter" && pickHunk(entry.hunk)}>{entry.text}</span
                >
                {#if onexpand && hidden[entry.hunk]}
                  <button
                    type="button"
                    class="expand mono"
                    title="Click for 20 more lines, Ctrl+click for the whole file"
                    onclick={(event) => onexpand?.(event.ctrlKey || event.metaKey)}
                    >▲ {hidden[entry.hunk]} lines hidden ▲</button
                  >
                {/if}
                {@render hunkActions(entry.hunk)}
              {:else if entry.row.kind === "context"}
                <span class="gutter"></span>
                <span class="num">{entry.row.old}</span>
                <span class="num">{entry.row.new}</span>
                <span class="code mono"
                  > {#each mergePieces(entry.row.text, tokensFor(entry.row), [], spansFor(rowIndex, "left")) as piece, i (i)}<span
                      class={piece.cls}
                      class:hit={piece.hit}
                      class:current={isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                    >{/each}</span
                >
              {:else if entry.row.kind === "delete"}
                <span
                  class="gutter"
                  class:picked={selected.has("d:" + entry.row.old)}
                  role="button"
                  tabindex="-1"
                  onclick={() => pick(entry.row)}
                  onkeydown={(e) => e.key === "Enter" && pick(entry.row)}
                  >{stageable ? (selected.has("d:" + entry.row.old) ? "■" : "□") : ""}</span
                >
                <span class="num">{entry.row.old}</span>
                <span class="num"></span>
                <span class="code mono del" class:moved={entry.row.moved}
                  >−{#each mergePieces(entry.row.text, tokensFor(entry.row), entry.row.inline, spansFor(rowIndex, "left")) as piece, i (i)}<span
                      class="{piece.cls}"
                      class:word={piece.changed}
                      class:hit={piece.hit}
                      class:current={isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                    >{/each}</span
                >
              {:else if entry.row.kind === "insert"}
                <span
                  class="gutter"
                  class:picked={selected.has("i:" + entry.row.new)}
                  role="button"
                  tabindex="-1"
                  onclick={() => pick(entry.row)}
                  onkeydown={(e) => e.key === "Enter" && pick(entry.row)}
                  >{stageable ? (selected.has("i:" + entry.row.new) ? "■" : "□") : ""}</span
                >
                <span class="num"></span>
                <span class="num">{entry.row.new}</span>
                <span class="code mono add" class:moved={entry.row.moved}
                  >+{#each mergePieces(entry.row.text, tokensFor(entry.row), entry.row.inline, spansFor(rowIndex, "left")) as piece, i (i)}<span
                      class="{piece.cls}"
                      class:word={piece.changed}
                      class:hit={piece.hit}
                      class:current={isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                    >{/each}</span
                >
              {/if}
            </div>
          {/each}
        {:else}
          {#each split.slice(range.start, range.end) as entry, index (range.start + index)}
            {@const rowIndex = range.start + index}
            <div class="line" style:top="{rowIndex * ROW_HEIGHT}px">
              {#if entry.header}
                <span class="header mono">{entry.header}</span>
                {@render hunkActions(entry.hunk)}
              {:else if entry.pair}
                <span class="num">{entry.pair.left?.line ?? ""}</span>
                <span
                  class="code mono side"
                  class:del={entry.pair.left?.kind === "delete"}
                  class:moved={entry.pair.left?.moved}
                  >{sign(entry.pair.left)}{#each cells(entry.pair.left, rowIndex, "left") as piece, i (i)}<span
                      class="{piece.cls}"
                      class:word={piece.changed}
                      class:hit={piece.hit}
                      class:current={isCurrent(rowIndex, "left", piece.start)}>{piece.text}</span
                    >{/each}</span
                >
                <span class="gap"></span>
                <span class="num">{entry.pair.right?.line ?? ""}</span>
                <span
                  class="code mono side"
                  class:add={entry.pair.right?.kind === "insert"}
                  class:moved={entry.pair.right?.moved}
                  >{sign(entry.pair.right)}{#each cells(entry.pair.right, rowIndex, "right") as piece, i (i)}<span
                      class="{piece.cls}"
                      class:word={piece.changed}
                      class:hit={piece.hit}
                      class:current={isCurrent(rowIndex, "right", piece.start)}>{piece.text}</span
                    >{/each}</span
                >
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .diff {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
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
    height: 20px;
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

  .bar button:hover {
    background: var(--state-hover);
  }

  .scroll {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .rows {
    position: relative;
  }

  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    height: 18px;
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
    background: rgb(90 160 110 / 14%);
    box-shadow: inset 2px 0 0 var(--status-add);
  }

  .acts {
    display: flex;
    gap: var(--sp-2);
    flex: 0 0 auto;
    padding-right: var(--sp-3);
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

  .acts button:hover {
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
    height: 20px;
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

  .header.clickable:hover {
    color: var(--status-add);
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
    text-overflow: ellipsis;
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
    fill: var(--c-added-bg, rgb(40 80 45 / 35%));
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
    background: rgb(255 255 255 / 14%);
    font-weight: 600;
  }

  .find {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-2) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    background: var(--surface-raised);
  }

  .find input {
    flex: 1 1 auto;
    min-width: 0;
    height: 22px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .find button {
    height: 20px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .find button:disabled {
    color: var(--text-secondary);
  }

  .count {
    flex: 0 0 auto;
    min-width: 64px;
    color: var(--text-secondary);
    font-size: 11px;
    text-align: right;
  }

  /* Every match is marked; the one the counter points at is the bright one. */
  .hit {
    border-radius: 2px;
    background: rgb(220 180 60 / 30%);
  }

  .hit.current {
    background: rgb(240 200 70 / 75%);
    color: #1a1a1a;
  }

  /* A moved block is one fact, not a deletion plus an addition (T7.9). */
  .code.moved {
    background: var(--c-stash-bg, rgb(70 60 95 / 35%));
    color: var(--status-stash);
  }

  .code.del {
    background: var(--c-deleted-bg, rgb(90 40 40 / 35%));
    color: var(--status-delete);
  }

  .code.add {
    background: var(--c-added-bg, rgb(40 80 45 / 35%));
    color: var(--status-add);
  }

  .expand {
    margin-left: var(--sp-4);
    padding: 0 var(--sp-3);
    background: var(--surface-raised);
    color: var(--text-secondary);
    border: 0;
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-header);
    cursor: default;
  }

  .expand:hover {
    color: var(--text-primary);
  }

  .header {
    flex: 1 1 auto;
    padding-left: var(--sp-4);
    color: var(--status-ref);
    background: var(--surface-raised);
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
