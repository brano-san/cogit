<script lang="ts">
  import { errors } from "$stores/errors.svelte";
  import { diffKey } from "$lib/diff-keys";
  import { modals } from "$lib/modal-stack";
  import { ON_MAC, primary } from "$lib/platform";
  import { untrack } from "svelte";
  import type { SearchRow } from "$lib/diff-rows";
  import { diffTokens, paneTokens, unifiedTokens } from "$lib/diff-highlight";
  import { FLASH_MS, changeAt, foldDiff, navState, revealRange, type Gap, type LineRange } from "$lib/diff-fold";
  import {
    buildBlocks,
    changeRow,
    changes,
    hunkKeys,
    type BlockKind,
    type PaneLine,
    type PaneRow,
    type UnifiedLine,
  } from "$lib/diff-blocks";
  import { alignModel } from "$lib/diff-aligned";
  import { DiffSearch } from "$lib/diff-search.svelte";
  import {
    ACTION_CENTER_X,
    ACTION_LEFT_X,
    ACTION_RIGHT_X,
    BAND_WIDTH,
    GUTTER_WIDTH,
    NUM_WIDTH,
    SPLIT_EVEN,
    SPLIT_MAX,
    SPLIT_MIN,
    SPLIT_STEP,
    bandLeft,
    blockConnectors,
    clampShare,
    draggedShare,
    type PaneView,
  } from "$lib/diff-band";
  import { mapLeftScrollToRight, mapRightScrollToLeft, wheelScroll } from "$lib/diff-sync";
  import { ScrollGuard, type PaneSide } from "$lib/diff-scroll-guard";
  import { sideCaptions, type SideCaptions } from "$lib/compare-params";
  import { settings } from "$stores/settings.svelte";
  import DiffFindBar from "./DiffFindBar.svelte";
  import FileSummary from "./FileSummary.svelte";
  import { binaryReason, tooLargeReason } from "$lib/diff-summary";
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
  import {
    alignedButton,
    bandAction,
    eolChangeText,
    eolLabel,
    layoutTip,
    modeChangeText,
    whitespaceButton,
    WHITESPACE_SHOWN_LABEL,
  } from "$lib/diff-toolbar";
  import { loadLanguage, mergePieces } from "$lib/highlight";
  import { toggleLine } from "$lib/selection";
  import { discardsWholeNewFile, keepSelection } from "$lib/diff-selection";
  import { investigateTarget, openInvestigate } from "$lib/investigate/open";
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
    /** Whether the window's keys are the diff's (11 §7): in the main window only while the
        Diff panel has the focus; a window of its own has nothing else to give them to. */
    active?: boolean;
    /** Edit in place (F-722): `null` or absent shows no button; `blocked` says why it is off. */
    edit?: { blocked: string | null } | null;
    /** `at`: a click in the right pane put the caret there. */
    onedit?: (at?: { line: number; column: number }) => void;
    /** Off where the file is named already: the compare window's title and header. */
    showPath?: boolean;
    /** What the panes hold, above each; from the diff's spec when the host knows no better
        (the compare window knows the parent commit's id). */
    captions?: SideCaptions;
  }

  let {
    diff,
    path,
    stageable: stageableFile = false,
    onstage,
    onblame,
    whitespace = "none",
    onwhitespace,
    onexpand,
    active = true,
    showPath = true,
    captions,
    edit = null,
    onedit,
  }: Props = $props();

  /** The right pane of a working-tree diff is the file: a plain click there (not a drag that
      selects) opens the editor with the caret at that line and column, as in SmartGit. */
  function clickToEdit(event: MouseEvent) {
    if (!edit || edit.blocked !== null || !onedit || event.button !== 0) return;
    if (!(window.getSelection()?.isCollapsed ?? true)) return;
    const row = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-line]");
    if (!row) return;
    const text = row.querySelector<HTMLElement>(".text");
    let column = 0;
    const caret = document.caretPositionFromPoint?.(event.clientX, event.clientY);
    if (text && caret && text.contains(caret.offsetNode)) {
      const range = document.createRange();
      range.setStart(text, 0);
      range.setEnd(caret.offsetNode, caret.offset);
      column = range.toString().length;
    }
    onedit({ line: Number(row.dataset.line), column });
  }

  /** Converted lines are not the file's bytes: a patch built from them would not apply. */
  const stageable = $derived(stageableFile && !(diff.kind === "text" && diff.converted));

  const wsButton = $derived(whitespaceButton(whitespace));

  let selected = $state<Set<string>>(new Set());

  const ROW_HEIGHT = 18;
  const BUFFER_ROWS = 12;
  /** Rows of context a jump leaves above the change it lands on. */
  const LEAD = 3;
  /** The height of a button in the gutter: its anchor keeps half of it clear of the edges. */
  const ACT_HEIGHT = 18;

  const mode = $derived(diffStore.layout);
  /** Side by side only: both sides on the same rows, or each side's own (08 §12.2). */
  const sideLayout = $derived(settings.current.diffLayout);
  const alignedTool = $derived(alignedButton(sideLayout));
  let findBar: ReturnType<typeof DiffFindBar> | undefined = $state();
  let unifiedEl: HTMLDivElement | undefined = $state();
  let leftEl: HTMLDivElement | undefined = $state();
  let rightEl: HTMLDivElement | undefined = $state();
  /** Each scroller's own position. Side by side the panes differ: the lines of a change
      are fewer on one side, and `diff-sync` maps one onto the other. */
  let uTop = $state(0);
  let lTop = $state(0);
  let rTop = $state(0);
  let viewportHeight = $state(0);
  /** The change the arrows and F6 step from; `-1` above the first one (#13). */
  let current = $state(-1);
  /** Set while a jump's own scroll is in flight, so it does not re-derive `current`. */
  let jumping = false;
  /** Lines the user opened out of the folds, by old line number (#16). */
  let revealed = $state<LineRange[]>([]);

  const hunks = $derived<Hunk[]>(diff.kind === "text" ? diff.hunks : []);
  const language = $derived(diff.kind === "text" ? diff.language : null);
  let grammar = $state<string | null>(null);
  $effect(() => {
    const wanted = language;
    void loadLanguage(wanted).then(() => (grammar = wanted));
  });

  const entries = $derived(
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

  /** Whether a change of indentation alone is marked: only while it is not ignored. */
  const indent = $derived(wsButton.indent);
  /** The blocks of the file (08 §12): each pane's own rows, the unified rows. */
  const base = $derived(buildBlocks(entries, { indent }));
  /** What is drawn: side by side aligned it is the same blocks on shared rows, with filler. */
  const model = $derived(mode === "split" && sideLayout === "aligned" ? alignModel(base) : base);

  /** Parsed once per diff, per side: a block comment must survive the line it opened on. */
  const tokens = $derived(
    diffTokens(
      {
        hunks,
        language: grammar === language ? language : null,
        oldText: diff.kind === "text" ? diff.oldText : null,
        newText: diff.kind === "text" ? diff.newText : null,
      },
      () => entries,
    ),
  );

  const unifiedRange = $derived(visibleRange(uTop, viewportHeight, ROW_HEIGHT, model.unified.length, BUFFER_ROWS));
  const leftRange = $derived(visibleRange(lTop, viewportHeight, ROW_HEIGHT, model.left.length, BUFFER_ROWS));
  const rightRange = $derived(visibleRange(rTop, viewportHeight, ROW_HEIGHT, model.right.length, BUFFER_ROWS));

  const changeList = $derived(changes(model));
  /** Where each change starts in the list a jump scrolls: the left pane, or the unified one. */
  const starts = $derived(changeList.map((change) => changeRow(model, change, mode === "unified")));
  const nav = $derived(navState(starts.length, current));
  const navTop = $derived(mode === "unified" ? uTop : lTop);

  /** The blocks of the change a jump landed on, lit briefly so the eye finds them (F-541). */
  let flash = $state<{ from: number; to: number } | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;

  function flashChange(index: number) {
    const change = changeList[index];
    if (!change) return;
    clearTimeout(flashTimer);
    flash = null;
    // A frame without the class, so the same rows lit again start their fade over.
    requestAnimationFrame(() => {
      flash = { ...change };
      flashTimer = setTimeout(() => (flash = null), FLASH_MS);
    });
  }

  function flashed(row: { block: number; blockKind: BlockKind }): boolean {
    return flash !== null && row.blockKind !== "equal" && row.block >= flash.from && row.block <= flash.to;
  }

  let rowsWidth = $state(0);
  /** The sign column, measured: it is sized in `ch` of the code font. */
  let signWidth = $state(0);
  /** Side by side, while the divider is dragged; the settings hold it once let go (R-535). */
  let dragShare = $state<number | null>(null);
  let dragFrom: { x: number; share: number } | null = null;
  const share = $derived(dragShare ?? settings.current.diffSplit);
  const sideWidth = $derived(GUTTER_WIDTH + NUM_WIDTH + signWidth);

  const left = $derived(bandLeft(rowsWidth, share, sideWidth));

  function saveShare(next: number) {
    void settings.set("diffSplit", clampShare(next));
  }

  function ondividerdown(event: PointerEvent) {
    if (event.button !== 0) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragFrom = { x: event.clientX, share };
    dragShare = share;
    event.preventDefault();
  }

  function ondividermove(event: PointerEvent) {
    if (!dragFrom) return;
    dragShare = draggedShare(dragFrom.share, event.clientX - dragFrom.x, rowsWidth, sideWidth);
  }

  function ondividerup(event: PointerEvent) {
    const dropped = dragShare;
    if (!dragFrom) return;
    dragFrom = null;
    if (dropped !== null) saveShare(dropped);
    dragShare = null;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  }

  function ondividerkey(event: KeyboardEvent) {
    if (event.key === "ArrowLeft") saveShare(share - SPLIT_STEP);
    else if (event.key === "ArrowRight") saveShare(share + SPLIT_STEP);
    else if (event.key === "Home") saveShare(SPLIT_EVEN);
    else return;
    event.preventDefault();
  }

  /** What the band's » and × do: throw away in the working tree. The staged diff has none
      (Unstage is on its hunks and in the toolbar). */
  const bandActions = $derived(bandAction(stageable, diffStore.lineActions));

  function bandAct(keys: readonly string[], label: string) {
    askDiscard(new Set(keys), label);
  }

  /** The connectors of the blocks near the viewport, from both panes' live scroll (08 §12).
      The scroll events that move `lTop` and `rTop` are once a frame and the state is read by
      the same flush that moves the rows, so the band and the rows share a frame. */
  const links = $derived(
    mode === "split"
      ? blockConnectors(
          model,
          { scrollTop: lTop, viewport: viewportHeight },
          { scrollTop: rTop, viewport: viewportHeight },
          ROW_HEIGHT,
          ACT_HEIGHT / 2,
        )
      : [],
  );

  /** Only code is searchable: a hit on a fold would scroll to nothing useful. Side by side
      a row index is a row of that side's own pane. */
  const searchTexts = $derived.by<SearchRow[]>(() => {
    if (mode === "unified") {
      return model.unified.map((row) => [row.kind === "line" ? row.text : null, null]);
    }
    const text = (row: PaneRow | undefined) => (row?.kind === "line" ? row.text : null);
    return Array.from({ length: Math.max(model.left.length, model.right.length) }, (_, i) => [
      text(model.left[i]),
      text(model.right[i]),
    ]);
  });

  const find = new DiffSearch(() => searchTexts);

  /** Twenty is not enough: `offsetWidth` is whole pixels, and the error adds up per character. */
  const PROBE = "0".repeat(100);
  /** Measured in the hidden ruler row, which has the layout of every other row. */
  let codeWidth = $state(0);
  /** Side by side, the right code column: no longer as wide as the left one (R-535). */
  let rightWidth = $state(0);
  let probeWidth = $state(0);
  const charWidth = $derived(probeWidth / PROBE.length);
  /** Pixels the code of every column is moved left by; the numbers stay put. */
  let sideways = $state(0);

  /** The widest line of each side: side by side, each column scrolls within its own width. */
  const widest = $derived.by(() => {
    const columns = (rows: PaneRow[]) => {
      let most = 0;
      for (const row of rows) {
        if (row.kind === "line") most = Math.max(most, textColumns(row.text) + (row.noNewline ? NO_NEWLINE_COLUMNS : 0));
      }
      return most + TRAILING_COLUMNS;
    };
    return { old: columns(model.left), new: columns(model.right) };
  });
  const sidewaysMax = $derived(
    mode === "split"
      ? Math.max(maxOffset(widest.old, charWidth, codeWidth), maxOffset(widest.new, charWidth, rightWidth))
      : maxOffset(Math.max(widest.old, widest.new), charWidth, codeWidth),
  );
  const shift = $derived(clampOffset(sideways, sidewaysMax));

  const paneEl = (side: PaneSide) => (side === "left" ? leftEl : rightEl);

  // --- synchronized scrolling: either pane drives the other through `diff-sync` ---

  const guard = new ScrollGuard();

  function viewOf(el: HTMLElement): PaneView {
    return { scrollTop: el.scrollTop, viewport: el.clientHeight };
  }

  /** Brings the other pane to where `from` is. The write is remembered, so the scroll event
      it raises is not mapped back. */
  function sync(from: PaneSide) {
    if (!leftEl || !rightEl) return;
    const l = viewOf(leftEl);
    const r = viewOf(rightEl);
    const target = from === "left" ? mapLeftScrollToRight(model, l, r, ROW_HEIGHT) : mapRightScrollToLeft(model, l, r, ROW_HEIGHT);
    const other: PaneSide = from === "left" ? "right" : "left";
    const el = paneEl(other)!;
    if (guard.write(other, el.scrollTop, target)) el.scrollTop = target;
    lTop = leftEl.scrollTop;
    rTop = rightEl.scrollTop;
  }

  function onpane(side: PaneSide) {
    const el = paneEl(side);
    if (!el) return;
    if (side === "left") lTop = el.scrollTop;
    else rTop = el.scrollTop;
    if (guard.echo(side, el.scrollTop)) return;
    sync(side);
    if (jumping) jumping = false;
    else settle();
  }

  /** The wheel over the gutter: one delta, both panes (each stops at its own end). */
  function onbandwheel(event: WheelEvent) {
    if (!leftEl || !rightEl) return;
    event.preventDefault();
    const side = wheelSideways(event, ROW_HEIGHT, viewportHeight);
    if (side !== 0) {
      if (sidewaysMax > 0) sideways = clampOffset(shift + side, sidewaysMax);
      return;
    }
    const scale = event.deltaMode === 1 ? ROW_HEIGHT : event.deltaMode === 2 ? viewportHeight : 1;
    const next = wheelScroll(model, viewOf(leftEl), viewOf(rightEl), event.deltaY * scale, ROW_HEIGHT);
    for (const [side, el, top] of [
      ["left", leftEl, next.left],
      ["right", rightEl, next.right],
    ] as const) {
      if (guard.write(side, el.scrollTop, top)) el.scrollTop = top;
    }
    lTop = leftEl.scrollTop;
    rTop = rightEl.scrollTop;
  }

  /** The row index to centre in the pane that holds it; the other pane follows. */
  function scrollToRow(index: number, side: PaneSide = "left") {
    const el = mode === "unified" ? unifiedEl : paneEl(side);
    if (!el) return;
    el.scrollTop = Math.max(index * ROW_HEIGHT - Math.floor(viewportHeight / 2), 0);
  }

  /** Down to the hit the counter points at, and sideways when it is past an edge. */
  function revealHit() {
    const hit = find.current;
    if (!hit) return;
    scrollToRow(hit.index, hit.side);
    const text = searchTexts[hit.index]?.[hit.side === "left" ? 0 : 1];
    if (text === null || text === undefined || charWidth === 0) return;
    const from = textColumns(text.slice(0, hit.from)) * charWidth;
    const to = textColumns(text.slice(0, hit.to)) * charWidth;
    const view = hit.side === "right" ? rightWidth : codeWidth;
    sideways = revealOffset(shift, view, from, to, sidewaysMax, REVEAL_MARGIN_COLUMNS * charWidth);
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

  /** The hunk a fold row opens onto, for the Stage, Unstage and Discard it carries. */
  function hunkBelow(rows: readonly { kind: string; block: number }[], index: number): number | null {
    const next = rows[index + 1];
    return next && next.kind !== "gap" ? model.blocks[next.block]!.hunk : null;
  }

  /** Ctrl+click opens every fold at once; a fold whose lines are not here asks for them. */
  function openGap(gap: Gap, how: "up" | "down" | "all", event: MouseEvent) {
    const whole = primary(event, ON_MAC);
    revealed = [...revealed, whole ? { from: 1, to: Number.MAX_SAFE_INTEGER } : revealRange(gap, how)];
    if (gap.loaded) return;
    if (onexpand) onexpand(true);
    else if (diffStore.repo !== null) void diffStore.expand(diffStore.repo, true);
  }

  /** Selecting works on any diff: a commit cannot be staged, but it can be investigated. */
  function pick(key: string | null) {
    if (key) selected = toggleLine(selected, key);
  }

  function pickHunk(hunk: number) {
    const keys = hunkKeys(model, hunk);
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
  function applyHunk(hunk: number, reverse: boolean) {
    onstage?.(hunkKeys(model, hunk), reverse);
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
  let pendingDiscard = $state.raw<{ keys: Set<string>; label: string; of: FileDiff; wholeFile: boolean } | null>(null);

  function askDiscard(keys: Set<string>, label: string) {
    if (keys.size === 0) return;
    const wholeFile = diff.kind === "text" && discardsWholeNewFile(diff, keys);
    pendingDiscard = { keys, label, of: diff, wholeFile };
  }

  async function confirmDiscard() {
    const pending = pendingDiscard;
    if (!pending) return;
    pendingDiscard = null;
    try {
      await diffStore.discardLines(pending.keys, pending.of);
      selected = new Set();
    } catch (err) {
      errors.report(err, "Could not discard the lines", path);
    }
  }

  function settle() {
    const top = Math.floor(navTop / ROW_HEIGHT);
    const bottom = Math.floor((navTop + viewportHeight) / ROW_HEIGHT) - 1;
    current = changeAt(starts, top, bottom, LEAD);
  }

  function jump(delta: number) {
    const target = current + delta;
    const el = mode === "unified" ? unifiedEl : leftEl;
    if (!el || target < 0 || target >= starts.length) return;
    const before = el.scrollTop;
    el.scrollTop = Math.max((starts[target] ?? 0) - LEAD, 0) * ROW_HEIGHT;
    jumping = el.scrollTop !== before;
    current = target;
    flashChange(target);
  }

  function onunified() {
    if (!unifiedEl) return;
    uTop = unifiedEl.scrollTop;
    if (jumping) jumping = false;
    else settle();
  }

  /** Listens in the capture phase, ahead of the main window's own F6: a key the diff takes
      is marked handled before the panel walk looks at it. */
  function onkeydown(event: KeyboardEvent) {
    // A dialog above the panel has the keys (11 §1).
    if (modals.any) return;
    const press = {
      key: event.key,
      code: event.code,
      ctrl: primary(event, ON_MAC),
      shift: event.shiftKey,
      alt: event.altKey,
    };
    const action = diffKey(press, { active, findShowing: find.showing, prev: nav.prev, next: nav.next });
    if (action === null) return;
    event.preventDefault();
    if (action.kind === "jump") jump(action.by);
    else if (action.kind === "layout") void diffStore.setLayout(mode === "split" ? "unified" : "split");
    else if (action.kind === "find") {
      if (find.showing) find.close();
      else openFind();
    }
    else if (action.kind === "investigate") startInvestigate();
    else find.close();
  }

  $effect(() => {
    void diffStore.loadPreferences();
  });

  // The geometry of the band is read from the panes' scroll and size, so both are re-read
  // whenever either pane is resized: a window or a divider drag clamps scrollTop without the
  // band hearing of it otherwise (R-628 c).
  $effect(() => {
    const els = [unifiedEl, leftEl, rightEl].filter((el): el is HTMLDivElement => !!el);
    if (els.length === 0) return;
    const observer = new ResizeObserver(() => {
      viewportHeight = (unifiedEl ?? leftEl)?.clientHeight ?? 0;
      if (leftEl) lTop = leftEl.scrollTop;
      if (rightEl) rTop = rightEl.scrollTop;
      if (unifiedEl) uTop = unifiedEl.scrollTop;
    });
    for (const el of els) observer.observe(el);
    return () => observer.disconnect();
  });

  // Another layout is another set of scrollers, each starting at the top.
  $effect(() => {
    void mode;
    void sideLayout;
    untrack(() => {
      uTop = lTop = rTop = 0;
      current = -1;
    });
  });

  // A new model (a fold opened, another whitespace mode) moves every anchor after it: the
  // right pane is put back where the left one is.
  $effect(() => {
    void model;
    void leftEl;
    void rightEl;
    untrack(() => {
      if (mode === "split") sync("left");
    });
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
    sideways = 0;
    flash = null;
    for (const el of [unifiedEl, leftEl, rightEl]) if (el) el.scrollTop = 0;
  });

  $effect(() => () => clearTimeout(flashTimer));

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

  // --- what a row looks like ---

  type Tone = "del" | "add" | "changed" | "move" | "";

  /** Colors go by meaning: red is what is gone, green what is new, violet what moved. Side by
      side (`joint`) a changed block is orange on both sides, every line of it; unified keeps its
      lines apart, red and green. */
  function toneOf(kind: BlockKind, deleting: boolean, joint = false): Tone {
    if (kind === "equal") return "";
    if (kind === "moved") return "move";
    return kind === "changed" && joint ? "changed" : deleting ? "del" : "add";
  }

  function paneLine(side: PaneSide, row: PaneLine) {
    const block = model.blocks[row.block]!;
    // An unchanged line quotes the old text on both sides; its twin has the other number.
    const twin =
      row.blockKind === "equal"
        ? side === "left"
          ? block.rightStart + row.line - block.leftStart
          : block.leftStart + row.line - block.rightStart
        : null;
    return paneTokens(tokens, side, row.line, row.text, twin);
  }

  const captionsShown = $derived(captions ?? (diffStore.spec ? sideCaptions(diffStore.spec) : null));
</script>

<svelte:window onkeydowncapture={onkeydown} />

{#snippet gutterCell(key: string | null)}
  <span
    class="gutter"
    class:picked={key !== null && selected.has(key)}
    role="button"
    tabindex="-1"
    onclick={() => pick(key)}
    onkeydown={(e) => e.key === "Enter" && pick(key)}
    >{stageable && key !== null ? (selected.has(key) ? "■" : "□") : ""}</span
  >
{/snippet}

{#snippet hunkActions(hunk: number)}
  {#if stageable}
    <span class="acts">
      <button type="button" title="Select every changed line of this block" onclick={() => pickHunk(hunk)}
        >Select</button
      >
      <button
        type="button"
        title="Stage this block"
        disabled={!diffStore.lineActions.stage}
        onclick={() => applyHunk(hunk, false)}>Stage</button
      >
      <button
        type="button"
        title="Unstage this block"
        disabled={!diffStore.lineActions.unstage}
        onclick={() => applyHunk(hunk, true)}>Unstage</button
      >
      {#if diffStore.lineActions.discard}
        <button
          type="button"
          class="danger"
          title="Throw this block away (always asks first)"
          onclick={() => askDiscard(hunkKeys(model, hunk), "this block")}>Discard</button
        >
      {/if}
    </span>
  {/if}
{/snippet}

<!-- A fold takes one row of the same height as a line of code (R-626): the anchors of the
     blocks after it stay on the row grid. Unified and the left pane carry the controls; the
     right pane's strip carries the hunk's actions, where its header used to put them. -->
{#snippet fold(gap: Gap, hunk: number | null, role: "both" | "left" | "right")}
  <div class="fold" role="group" aria-label="{gap.hidden} lines hidden">
    {#if role !== "right"}
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
    {/if}
    {#if role !== "left" && hunk !== null}{@render hunkActions(hunk)}{/if}
  </div>
{/snippet}

{#snippet pane(side: PaneSide, rows: PaneRow[], range: { start: number; end: number })}
  {#each rows.slice(range.start, range.end) as row, k (range.start + k)}
    {@const index = range.start + k}
    {#if row.kind === "gap"}
      <div class="line" style:top="{index * ROW_HEIGHT}px">
        {@render fold(row.gap, hunkBelow(rows, index), side)}
      </div>
    {:else if row.kind === "filler"}
      <div class="line" style:top="{index * ROW_HEIGHT}px" aria-hidden="true">
        <span class="filler"></span>
      </div>
    {:else}
      {@const tone = toneOf(row.blockKind, side === "left", true)}
      {@const picked = row.key !== null && selected.has(row.key)}
      <div
        class="line"
        class:staging={stageable && picked}
        class:marked={!stageable && picked}
        class:flash={flashed(row)}
        style:top="{index * ROW_HEIGHT}px"
        data-line={side === "right" ? row.line : undefined}
      >
        {#if side === "left"}{@render gutterCell(row.key)}{/if}
        <span class="num {tone}">{row.line}</span>
        <span class="sign {tone}">{tone === "" ? "" : side === "left" ? "−" : "+"}</span>
        <span class="code mono {tone}"
          ><span class="text"
            >{#each mergePieces(row.text, paneLine(side, row), row.inline, find.spansFor(index, side)) as piece, i (i)}<span
                class={piece.cls}
                class:word={piece.changed}
                class:hit={piece.hit}
                class:current={find.isCurrent(index, side, piece.start)}>{piece.text}</span
              >{/each}</span
          ></span
        >
        {#if side === "right"}{@render gutterCell(row.key)}{/if}
      </div>
    {/if}
  {/each}
{/snippet}

{#snippet unifiedRow(row: UnifiedLine, index: number)}
  {@const tone = toneOf(row.blockKind, row.type === "delete")}
  {@const picked = row.key !== null && selected.has(row.key)}
  <div
    class="line"
    class:staging={stageable && picked}
    class:marked={!stageable && picked}
    class:flash={flashed(row)}
    style:top="{index * ROW_HEIGHT}px"
  >
    {@render gutterCell(row.key)}
    <span class="num {tone}">{row.old ?? ""}</span>
    <span class="num {tone}">{row.new ?? ""}</span>
    <span class="sign {tone}">{row.type === "delete" ? "−" : row.type === "insert" ? "+" : ""}</span>
    <span class="code mono {tone}"
      ><span class="text"
        >{#each mergePieces(row.text, unifiedTokens(tokens, row), row.inline, find.spansFor(index, "left")) as piece, i (i)}<span
            class={piece.cls}
            class:word={piece.changed}
            class:hit={piece.hit}
            class:current={find.isCurrent(index, "left", piece.start)}>{piece.text}</span
          >{/each}</span
      ></span
    >
  </div>
{/snippet}

<div class="diff">
  <div class="bar">
    {#if showPath}<span class="path mono truncate">{path}</span>{:else}<span class="grow"></span>{/if}
    {#if diff.kind === "text"}
      {@const eol = eolLabel(diff.eol, diff.oldTotal, diff.newTotal)}
      {#if eol.warn}
        <span class="badge-warning" title={eol.title}>{eol.text}</span>
      {:else}
        <span class="eol" title={eol.title}>{eol.text}</span>
      {/if}
      {#if diff.lossyEncoding}<span class="warn">not valid UTF-8</span>{/if}
      {#if diff.converted}<span class="warn" title="Shown converted; stage or discard the file whole">{diff.converted}</span>{/if}
      <button type="button" class="btn sm" disabled={!nav.prev} onclick={() => jump(-1)} title="Previous change (Shift+F6)"
        >▲</button
      >
      <button type="button" class="btn sm" disabled={!nav.next} onclick={() => jump(1)} title="Next change (F6)">▼</button>
      <button
        type="button"
        class="btn sm"
        aria-pressed={find.showing}
        title="Search the lines shown in this diff (Ctrl+F); open the folds to search the whole file"
        onclick={() => (find.showing ? find.close() : openFind())}>Find</button
      >
      {#if stageable}
        <span class="picked tabular">{selected.size ? `${selected.size} selected` : ""}</span>
        <button
          type="button"
          class="btn sm"
          disabled={selected.size === 0 || !diffStore.lineActions.stage}
          onclick={() => apply(false)}>Stage lines</button
        >
        <button
          type="button"
          class="btn sm"
          disabled={selected.size === 0 || !diffStore.lineActions.unstage}
          onclick={() => apply(true)}>Unstage lines</button
        >
        {#if diffStore.lineActions.discard}
          <button
            type="button"
            class="btn sm danger"
            disabled={selected.size === 0}
            title="Throw the selected lines away (always asks first)"
            onclick={() => askDiscard(new Set(selected), `${selected.size} selected lines`)}
            >Discard lines</button
          >
        {/if}
      {/if}
    {/if}
    <!-- A file the mode hides entirely still needs the button that shows it (F-067). -->
    {#if onwhitespace && (diff.kind === "text" || diff.kind === "whitespaceOnly")}
      <button
        type="button"
        class="btn sm"
        aria-pressed={wsButton.pressed}
        title={wsButton.title}
        onclick={() => onwhitespace(wsButton.next)}
        >{wsButton.label}</button
      >
    {/if}
    {#if edit && onedit}
      <button
        type="button"
        class="btn sm"
        disabled={edit.blocked !== null}
        title={edit.blocked ?? "Edit the working tree file here; the left side stays read-only"}
        onclick={() => onedit?.()}>Edit</button
      >
    {/if}
    {#if diff.kind === "text"}
      {#if onblame}
        <button type="button" class="btn sm" title="Annotate every line with its commit" onclick={() => onblame()}
          >Blame</button
        >
      {/if}
      <button
        type="button"
        class="btn sm"
        disabled={diffStore.repo === null}
        title="Trace where the lines came from, starting at the selected one (Ctrl+Alt+Shift+L)"
        onclick={startInvestigate}>Investigate</button
      >
      <button
        type="button"
        class="btn sm"
        aria-pressed={diffStore.showMoves}
        title="Show a moved block as one move; off: as an ordinary deletion plus addition"
        onclick={() => diffStore.setShowMoves(!diffStore.showMoves)}
      >
        Moves
      </button>
      <button
        type="button"
        class="btn sm"
        aria-pressed={alignedTool.pressed}
        title={alignedTool.title}
        onclick={() => settings.set("diffLayout", alignedTool.next)}
      >
        {alignedTool.label}
      </button>
      <button
        type="button"
        class="btn sm"
        aria-pressed={mode === "unified"}
        title={layoutTip(mode)}
        onclick={() => diffStore.setLayout(mode === "split" ? "unified" : "split")}
      >
        Unified
      </button>
    {/if}
  </div>

  {#if pendingDiscard}
    <ConfirmDialog
      title="Discard lines"
      message={pendingDiscard.wholeFile
        ? `Throw away every line of ${path}? The file will be deleted from disk. Undo can put it back.`
        : `Throw away ${pendingDiscard.label} in ${path}? Undo can put them back.`}
      confirm="Discard"
      warning
      onanswer={(yes) => (yes ? void confirmDiscard() : (pendingDiscard = null))}
    />
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
      Only whitespace changed, and {wsButton.label} hides it.
      {onwhitespace
        ? `Click ${wsButton.label} in the bar above until it reads ${WHITESPACE_SHOWN_LABEL} to see the diff.`
        : "Choose Show every change in Preferences ▸ Diff View ▸ Whitespace to see the diff."}
    </p>
  {:else if diff.kind === "eolOnly"}
    <p class="message">
      Only the line endings changed: {eolChangeText(diff.from, diff.to)}. The content is identical.
    </p>
  {:else if diff.kind === "binary"}
    <FileSummary reason={binaryReason(diff.cause)} old={diff.old} next={diff.new} />
  {:else if diff.kind === "image"}
    <p class="message">Image ({diff.mime}) — {diff.oldSize} bytes → {diff.newSize} bytes.</p>
  {:else if diff.kind === "tooLarge"}
    <FileSummary reason={tooLargeReason(diff.limit)} old={diff.old} next={diff.new} />
  {:else if mode === "unified"}
    <div class="scroll" data-select-text="diff" bind:this={unifiedEl} onscroll={onunified} {onwheel}>
      <div class="rows" style:height="{model.unified.length * ROW_HEIGHT}px" style:--shift="{shift}px">
        <div class="line ruler" aria-hidden="true">
          <span class="gutter"></span>
          <span class="num"></span>
          <span class="num"></span>
          <span class="sign" bind:offsetWidth={signWidth}></span>
          <span class="code mono" bind:clientWidth={codeWidth}
            ><span class="probe" bind:offsetWidth={probeWidth}>{PROBE}</span></span
          >
        </div>
        {#each model.unified.slice(unifiedRange.start, unifiedRange.end) as row, k (unifiedRange.start + k)}
          {@const index = unifiedRange.start + k}
          {#if row.kind === "gap"}
            <div class="line" style:top="{index * ROW_HEIGHT}px">
              {@render fold(row.gap, hunkBelow(model.unified, index), "both")}
            </div>
          {:else}
            {@render unifiedRow(row, index)}
          {/if}
        {/each}
      </div>
    </div>
    <SidewaysScrollbar offset={shift} max={sidewaysMax} onscroll={(offset) => (sideways = offset)} />
  {:else}
    {#if captionsShown}
      <div class="captions">
        <span class="caption" style:flex-basis="{Math.round(left)}px" title={captionsShown.left}
          >{captionsShown.left}</span
        >
        <span class="caption-gap"></span>
        <span class="caption grow" title={captionsShown.right}>{captionsShown.right}</span>
      </div>
    {/if}
    <div class="panes" style:--shift="{shift}px" bind:clientWidth={rowsWidth}>
      <div
        class="pane left"
        data-select-text="diff"
        style:flex-basis="{Math.round(left)}px"
        bind:this={leftEl}
        onscroll={() => onpane("left")}
        {onwheel}
      >
        <div class="rows" style:height="{model.left.length * ROW_HEIGHT}px">
          <div class="line ruler" aria-hidden="true">
            <span class="gutter"></span>
            <span class="num"></span>
            <span class="sign" bind:offsetWidth={signWidth}></span>
            <span class="code mono" bind:clientWidth={codeWidth}
              ><span class="probe" bind:offsetWidth={probeWidth}>{PROBE}</span></span
            >
          </div>
          {@render pane("left", model.left, leftRange)}
        </div>
      </div>
      <!-- The band between the panes is the divider (drag it, or arrows while focused), the
           connectors of the blocks are drawn over it, and one action set per block over those. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="band" onwheel={onbandwheel}>
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          class="divider"
          class:dragging={dragShare !== null}
          role="separator"
          tabindex="0"
          aria-label="Width of the old and the new side"
          aria-orientation="vertical"
          aria-valuenow={Math.round(share * 100)}
          aria-valuemin={Math.round(SPLIT_MIN * 100)}
          aria-valuemax={Math.round(SPLIT_MAX * 100)}
          title="Drag to change the width of the two sides; double-click for an even split"
          onpointerdown={ondividerdown}
          onpointermove={ondividermove}
          onpointerup={ondividerup}
          onlostpointercapture={() => {
            dragFrom = null;
            dragShare = null;
          }}
          onkeydown={ondividerkey}
          ondblclick={() => saveShare(SPLIT_EVEN)}
        ></div>
        <svg class="connectors" width={BAND_WIDTH} height={viewportHeight} aria-hidden="true">
          {#each links as link (link.block + ":" + link.kind + ":" + link.moveId)}
            <path class="fill {link.kind}" d={link.path} />
          {/each}
          <!-- Outlines after every fill, so a neighbor's fill never covers them. -->
          {#each links as link (link.block + ":" + link.kind + ":" + link.moveId)}
            <path class="edge {link.kind}" d={link.edges} />
          {/each}
        </svg>
        <div class="band-ui">
          {#each links as link (link.block + ":" + link.kind + ":" + link.moveId)}
            {@const block = model.blocks[link.block]!}
            {#if link.kind !== "moved" && bandActions}
              {#if link.kind === "changed"}
                {#if link.anchor.center !== null}
                  <button
                    type="button"
                    class="bandact"
                    style:left="{ACTION_CENTER_X}px"
                    style:top="{link.anchor.center}px"
                    title={bandActions === "discard"
                      ? "Revert the whole block: restore its deleted lines and remove its added lines (asks first)"
                      : "Unstage the whole block: its deleted and its added lines"}
                    onclick={() => bandAct([...block.keys.deletes, ...block.keys.inserts], "the whole changed block (its deleted and its added lines)")}>»</button
                  >
                {/if}
              {:else}
                {#if block.keys.deletes.length > 0 && link.anchor.left !== null}
                  <button
                    type="button"
                    class="bandact"
                    style:left="{ACTION_LEFT_X}px"
                    style:top="{link.anchor.left}px"
                    title={bandActions === "discard"
                      ? "Restore these deleted lines in the working tree (asks first)"
                      : "Unstage the deletion of these lines"}
                    onclick={() => bandAct(block.keys.deletes, "the deletion of these lines")}>»</button
                  >
                {/if}
                {#if block.keys.inserts.length > 0 && link.anchor.right !== null}
                  <button
                    type="button"
                    class="bandact"
                    style:left="{ACTION_RIGHT_X}px"
                    style:top="{link.anchor.right}px"
                    title={bandActions === "discard"
                      ? "Remove these added lines from the working tree (asks first)"
                      : "Unstage these added lines"}
                    onclick={() => bandAct(block.keys.inserts, "these added lines")}>×</button
                  >
                {/if}
              {/if}
            {/if}
          {/each}
        </div>
      </div>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div
        class="pane right"
        class:editable={edit?.blocked === null && onedit !== undefined}
        data-select-text="diff"
        bind:this={rightEl}
        onscroll={() => onpane("right")}
        onclick={clickToEdit}
        {onwheel}
      >
        <div class="rows" style:height="{model.right.length * ROW_HEIGHT}px">
          <div class="line ruler" aria-hidden="true">
            <span class="num"></span>
            <span class="sign"></span>
            <span class="code mono" bind:clientWidth={rightWidth}></span>
            <span class="gutter"></span>
          </div>
          {@render pane("right", model.right, rightRange)}
        </div>
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
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border);
    font-size: var(--fs-dense);
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
  }

  .eol {
    color: var(--fg-secondary);
    font-size: 11px;
  }

  /* A change that needs a word of explanation, not a control: the tooltip says what it is. */
  .badge-warning {
    padding: 0 var(--sp-3);
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    border-radius: var(--r-sm);
    font-size: 11px;
    line-height: 16px;
    cursor: default;
  }

  .warn {
    color: var(--status-warning);
    font-size: 11px;
  }

  /* 06 §6: a control that cannot act says so, and does not light up under the pointer. */
  .acts button:disabled {
    opacity: 0.4;
  }

  /* Unified: one scroller. Sideways the code moves by `--shift`, under the scrollbar below
     the rows (R-470). */
  .scroll {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
    background: var(--bg-editor);
  }

  .rows {
    position: relative;
  }

  /* The name of what each pane holds, above it: the panes' own widths, so each sits over its pane. */
  .captions {
    display: flex;
    flex: 0 0 auto;
    height: 22px;
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border);
    color: var(--fg-secondary);
    font-size: var(--fs-header);
    line-height: 21px;
  }

  .caption {
    flex: 0 0 auto;
    min-width: 0;
    padding: 0 var(--sp-4);
    box-sizing: border-box;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .caption.grow {
    flex: 1 1 0;
  }

  .caption-gap {
    flex: 0 0 42px;
    border-inline: 1px solid var(--border);
  }

  /* Side by side: two scrollers, kept level by `diff-sync`. Only the right one shows its bar. */
  .panes {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
    background: var(--bg-editor);
  }

  /* The working file: it reads as text one can type into. */
  .pane.editable .code {
    cursor: text;
  }

  .pane {
    min-width: 0;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .pane.left {
    flex: 0 0 auto;
    scrollbar-width: none;
  }

  .pane.left::-webkit-scrollbar {
    display: none;
  }

  .pane.right {
    flex: 1 1 0;
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
    color: var(--fg-secondary);
    font-size: 9px;
    text-align: center;
    cursor: default;
  }

  .gutter.picked {
    color: var(--status-success);
  }

  /* What the next Stage or Unstage will act on: the selected-row look, on the row and not
     just in the 14-pixel gutter, so the extent of it reads at a glance (T6.7). */
  .line.staging {
    box-shadow: inset 2px 0 0 var(--selected-bar);
  }

  .line.staging .gutter,
  .line.staging .num {
    background: var(--bg-selected);
  }

  /* A read-only diff can still be selected, for Investigate; it just stages nothing. */
  .line.marked {
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .line.marked .gutter,
  .line.marked .num {
    background: var(--bg-selected-inactive);
  }

  /* Over the row's own fills, fading out in `FLASH_MS`; with reduced motion it just stays
     lit that long. Feedback, not a transition: nothing waits for it. */
  .line.flash::after {
    content: "";
    position: absolute;
    inset: 0;
    background: var(--diff-jump-flash);
    pointer-events: none;
    opacity: 0;
    animation: jump-flash 600ms ease-out;
  }

  @keyframes jump-flash {
    from {
      opacity: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .line.flash::after {
      animation: none;
      opacity: 1;
    }
  }

  .acts {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
    margin-left: auto;
    padding-right: var(--sp-3);
  }

  .acts button {
    height: 14px;
    padding: 0 var(--sp-2);
    background: var(--bg-input);
    color: var(--fg-secondary);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    font-size: 9px;
    line-height: 12px;
    cursor: default;
  }

  .acts button:hover:not(:disabled) {
    color: var(--fg-primary);
  }

  .acts button.danger {
    color: var(--status-danger);
  }

  .acts button.danger:hover {
    border-color: var(--status-danger);
  }

  .grow {
    flex: 1 1 auto;
  }

  .picked {
    color: var(--status-success);
    font-size: 10px;
  }

  .num {
    flex: 0 0 auto;
    width: 44px;
    padding-right: var(--sp-3);
    color: var(--diff-line-number);
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
    color: var(--fg-primary);
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
    color: var(--fg-secondary);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    user-select: none;
  }

  /* Red is what is gone, green what is new, violet what moved: by the block's meaning, the
     same on both layouts. Numbers and signs sit on the gutter shade of their line. */
  .num.del,
  .sign.del {
    background: var(--diff-del-gutter);
  }

  .num.add,
  .sign.add {
    background: var(--diff-add-gutter);
  }

  .num.changed,
  .sign.changed {
    background: var(--diff-changed-gutter);
  }

  .num.move,
  .sign.move {
    background: var(--diff-move-line);
  }

  .code.del {
    background: var(--diff-del-line);
  }

  .code.add {
    background: var(--diff-add-line);
  }

  .code.changed {
    background: var(--diff-changed-line);
  }

  .code.move {
    background: var(--diff-move-line);
  }

  /* Aligned: the row opposite a side with no line there. Thin diagonal hatching on a calm
     ground; the tile is 6 px and a row 18, so the lines run on across rows without a seam.
     Not text: never selected, copied or searched. */
  .filler {
    flex: 1 1 auto;
    background-color: var(--diff-filler-bg);
    background-image: linear-gradient(
      45deg,
      transparent calc(50% - 0.5px),
      var(--diff-filler-hatch) calc(50% - 0.5px),
      var(--diff-filler-hatch) calc(50% + 0.5px),
      transparent calc(50% + 0.5px)
    );
    background-size: 6px 6px;
    user-select: none;
    pointer-events: none;
  }

  /* The band: the divider, the connectors over it, the buttons over those. Its width must
     match `BAND_WIDTH`. */
  .band {
    position: relative;
    flex: 0 0 42px;
    background: var(--diff-center-gutter-bg);
  }

  /* The panel divider's look (R-500): a hairline that lights under the pointer. The whole
     band is the grab zone. */
  .divider {
    position: absolute;
    inset: 0;
    z-index: 1;
    cursor: col-resize;
    outline: none;
  }

  .divider::before {
    content: "";
    position: absolute;
    inset-block: 0;
    left: calc(50% - var(--w-splitter) / 2);
    width: var(--w-splitter);
    background: var(--splitter-track);
    transition: background var(--t-fast) var(--ease-out);
  }

  .divider:hover::before,
  .divider.dragging::before,
  .divider:focus-visible::before {
    background: var(--splitter-active);
  }

  .connectors {
    position: absolute;
    top: 0;
    left: 0;
    z-index: 2;
    pointer-events: none;
  }

  .fill {
    fill: var(--diff-connector-fill);
    stroke: none;
  }

  /* The outline: one pixel, the two curves only; the sides sit on the panes' own edges. */
  .edge {
    fill: none;
    stroke: var(--diff-connector-stroke);
    stroke-width: 1;
    stroke-linecap: butt;
    shape-rendering: geometricPrecision;
  }

  /* A changed block is orange like its lines (R-629): the fill is the line tone, the outline the word tone. */
  .fill.changed {
    fill: var(--diff-changed-line);
  }

  .edge.changed {
    stroke: var(--diff-changed-word);
  }

  /* A move goes somewhere else in the file: it keeps the violet of its lines. */
  .fill.moved {
    fill: var(--diff-move-line);
  }

  .edge.moved {
    stroke: var(--diff-move-word);
  }

  .band-ui {
    position: absolute;
    inset: 0;
    z-index: 3;
    pointer-events: none;
  }

  /* » in the left half of the gutter, × in the right (a changed block: one » in the middle): each centred on its x and on the part of
     its block's connector that is on screen there (blockConnectors, R-628). */
  .bandact {
    position: absolute;
    transform: translate(-50%, -50%);
    width: 14px;
    height: 18px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--diff-center-gutter-action);
    font-size: 13px;
    font-weight: 700;
    line-height: 18px;
    text-align: center;
    pointer-events: auto;
    cursor: pointer;
  }

  .bandact:hover {
    color: var(--diff-center-gutter-action-hover);
  }

  /* The marked word is plain primary text on its own background: the syntax colour (.tok-*)
     is off inside a highlighted fragment, so the word reads at 4.5:1 in every theme. */
  .word {
    border-radius: 2px;
    font-weight: 600;
    color: var(--fg-primary);
  }

  .code.del .word {
    background: var(--diff-del-word);
  }

  .code.add .word {
    background: var(--diff-add-word);
  }

  .code.changed .word {
    background: var(--diff-changed-word);
  }

  .code.move .word {
    background: var(--diff-move-word);
  }

  /* Every match is marked; the one the counter points at is the bright one. */
  .hit {
    border-radius: 2px;
    background: var(--search-hit);
  }

  .hit.current {
    background: var(--search-current);
    color: var(--search-ink);
  }

  /* One band in each pane, saying what is hidden (#16). */
  .fold {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex: 1 1 auto;
    min-width: 0;
    padding-left: var(--sp-4);
    background: var(--diff-hunk-header-bg);
    box-shadow:
      inset 0 1px 0 var(--border),
      inset 0 -1px 0 var(--border);
    color: var(--diff-hunk-header-fg);
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
    color: var(--diff-hunk-header-fg);
    font: inherit;
    line-height: 14px;
    cursor: default;
  }

  .fold > button:hover {
    background: var(--bg-hover);
    color: var(--fg-primary);
  }

  .fold .arrow {
    font-size: 9px;
  }

  .where {
    min-width: 0;
    margin-left: var(--sp-3);
    color: var(--diff-hunk-header-fg);
    font-family: var(--font-mono);
  }

  .message.warn {
    color: var(--status-warning);
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--fg-secondary);
  }
</style>
