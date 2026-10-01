import { blockInline, type InlineOptions, type Range } from "./diff-inline";
import type { FoldEntry, Gap } from "./diff-fold";
import type { DiffRow } from "./ipc";

/**
 * The block model of a split diff (SmartGit's): a file is a list of blocks, each a range of
 * lines on the left and one on the right. Adjacent deletes and inserts with no equal line
 * between are ONE `changed` block. A pane renders only its own lines, one after another —
 * there is no filler to even the sides out; the gutter between them draws the connection
 * (diff-band.ts) and scrolling lines the panes up by block anchors (diff-sync.ts).
 *
 * Built once per diff (one pass, O(rows)); everything the view needs per frame is an index
 * into the lists below, never a rebuild.
 */

export type BlockKind = "equal" | "added" | "removed" | "changed" | "moved";

export interface Block {
  id: number;
  kind: BlockKind;
  /** 1-based first line; for an empty side, the line the block would sit before. */
  leftStart: number;
  leftLen: number;
  rightStart: number;
  rightLen: number;
  /** Where the block starts in each pane's row list, and how many rows it takes there.
      A fold takes one row on both sides whatever it hides. */
  leftRow: number;
  leftRows: number;
  rightRow: number;
  rightRows: number;
  unifiedRow: number;
  unifiedRows: number;
  /** A moved block: both halves share it. */
  moveId: number | null;
  /** The fold-block the rows came from (`FoldEntry.block`): what hunk actions are keyed by. */
  hunk: number;
  /** The change keys (`d:<old>` / `i:<new>`) the block's Stage/Discard/Select act on. */
  keys: { deletes: string[]; inserts: string[] };
  /** Set on the equal block that stands for a collapsed zone. */
  gap: Gap | null;
}

export interface PaneLine {
  kind: "line";
  line: number;
  text: string;
  block: number;
  blockKind: BlockKind;
  moveId: number | null;
  noNewline: boolean;
  key: string | null;
  /** Word ranges (UTF-16) painted over the line. */
  inline: Range[];
}

export interface PaneGap {
  kind: "gap";
  gap: Gap;
  block: number;
}

export type PaneRow = PaneLine | PaneGap;

export interface UnifiedLine {
  kind: "line";
  type: "context" | "delete" | "insert";
  old: number | null;
  new: number | null;
  text: string;
  block: number;
  blockKind: BlockKind;
  moveId: number | null;
  noNewline: boolean;
  key: string | null;
  inline: Range[];
}

export type UnifiedRow = UnifiedLine | PaneGap;

export interface MovePair {
  moveId: number;
  left: number | null;
  right: number | null;
}

export interface BlockModel {
  blocks: Block[];
  left: PaneRow[];
  right: PaneRow[];
  unified: UnifiedRow[];
  moves: MovePair[];
}

type Del = Extract<DiffRow, { kind: "delete" }>;
type Ins = Extract<DiffRow, { kind: "insert" }>;
interface Segment {
  key: string;
  dels: Del[];
  inss: Ins[];
}

export function buildBlocks(entries: readonly FoldEntry[], opts: InlineOptions = {}): BlockModel {
  const blocks: Block[] = [];
  const left: PaneRow[] = [];
  const right: PaneRow[] = [];
  const unified: UnifiedRow[] = [];
  const moves = new Map<number, MovePair>();
  let leftNext = 1;
  let rightNext = 1;

  const open = (kind: BlockKind, hunk: number, moveId: number | null = null): Block => {
    const b: Block = {
      id: blocks.length,
      kind,
      leftStart: leftNext,
      leftLen: 0,
      rightStart: rightNext,
      rightLen: 0,
      leftRow: left.length,
      leftRows: 0,
      rightRow: right.length,
      rightRows: 0,
      unifiedRow: unified.length,
      unifiedRows: 0,
      moveId,
      hunk,
      keys: { deletes: [], inserts: [] },
      gap: null,
    };
    blocks.push(b);
    return b;
  };
  const close = (b: Block) => {
    b.leftRows = left.length - b.leftRow;
    b.rightRows = right.length - b.rightRow;
    b.unifiedRows = unified.length - b.unifiedRow;
  };

  let equal: Block | null = null;
  let run: Segment[] = [];
  let runHunk = 0;

  const flushRun = () => {
    for (const seg of run) {
      const moved = seg.key !== "plain";
      const moveId = moved ? (seg.dels[0]?.moveId ?? seg.inss[0]?.moveId ?? null) : null;
      const kind: BlockKind = moved
        ? "moved"
        : seg.dels.length > 0 && seg.inss.length > 0
          ? "changed"
          : seg.dels.length > 0
            ? "removed"
            : "added";
      const b = open(kind, runHunk, moveId);
      if (seg.dels[0]) b.leftStart = seg.dels[0].old;
      if (seg.inss[0]) b.rightStart = seg.inss[0].new;
      const common = (row: Del | Ins) => ({
        kind: "line" as const,
        text: row.text,
        block: b.id,
        blockKind: kind,
        moveId: row.moveId ?? null,
        noNewline: row.noNewline ?? false,
        // Moved lines keep the engine's own ranges; the block-level pass is for `changed`.
        inline: moved ? ([...row.inline] as Range[]) : [],
      });
      for (const row of seg.dels) {
        const key = `d:${row.old}`;
        b.keys.deletes.push(key);
        left.push({ ...common(row), line: row.old, key });
        unified.push({ ...common(row), type: "delete", old: row.old, new: null, key });
      }
      for (const row of seg.inss) {
        const key = `i:${row.new}`;
        b.keys.inserts.push(key);
        right.push({ ...common(row), line: row.new, key });
        unified.push({ ...common(row), type: "insert", old: null, new: row.new, key });
      }
      b.leftLen = seg.dels.length;
      b.rightLen = seg.inss.length;
      leftNext = b.leftStart + b.leftLen;
      rightNext = b.rightStart + b.rightLen;
      close(b);

      if (kind === "changed") {
        const l = left.slice(b.leftRow, b.leftRow + b.leftRows) as PaneLine[];
        const r = right.slice(b.rightRow, b.rightRow + b.rightRows) as PaneLine[];
        const out = blockInline(
          l.map((x) => x.text),
          r.map((x) => x.text),
          opts,
        );
        if (out) {
          // Unified lists the block's deletes, then its inserts: the same ranges.
          l.forEach((x, i) => {
            x.inline = out.left[i]!;
            (unified[b.unifiedRow + i] as UnifiedLine).inline = x.inline;
          });
          r.forEach((x, i) => {
            x.inline = out.right[i]!;
            (unified[b.unifiedRow + l.length + i] as UnifiedLine).inline = x.inline;
          });
        }
      }
      if (moved && moveId !== null) {
        const pair = moves.get(moveId) ?? { moveId, left: null, right: null };
        if (seg.dels.length > 0) pair.left = b.id;
        else pair.right = b.id;
        moves.set(moveId, pair);
      }
    }
    run = [];
  };

  const closeEqual = () => {
    if (equal) close(equal);
    equal = null;
  };

  for (const entry of entries) {
    if (entry.kind === "gap") {
      flushRun();
      closeEqual();
      const g = entry.gap;
      leftNext = g.oldFrom;
      rightNext = g.newFrom;
      const b = open("equal", runHunk);
      b.gap = g;
      b.leftLen = b.rightLen = g.hidden;
      left.push({ kind: "gap", gap: g, block: b.id });
      right.push({ kind: "gap", gap: g, block: b.id });
      unified.push({ kind: "gap", gap: g, block: b.id });
      close(b);
      leftNext = g.oldFrom + g.hidden;
      rightNext = g.newFrom + g.hidden;
      continue;
    }
    const row = entry.row;
    if (row.kind === "context") {
      flushRun();
      if (!equal) {
        leftNext = row.old;
        rightNext = row.new;
        equal = open("equal", entry.block);
      }
      const common = {
        block: equal.id,
        blockKind: "equal" as const,
        moveId: null,
        noNewline: row.noNewline ?? false,
        key: null,
        inline: [] as Range[],
      };
      left.push({ kind: "line", line: row.old, text: row.text, ...common });
      right.push({ kind: "line", line: row.new, text: row.text, ...common });
      unified.push({ kind: "line", type: "context", old: row.old, new: row.new, text: row.text, ...common });
      equal.leftLen++;
      equal.rightLen++;
      leftNext = row.old + 1;
      rightNext = row.new + 1;
      continue;
    }
    closeEqual();
    if (run.length === 0) runHunk = entry.block;
    const key = row.moved === true ? `m:${row.kind}:${row.moveId ?? -1}` : "plain";
    let seg = run[run.length - 1];
    if (!seg || seg.key !== key) run.push((seg = { key, dels: [], inss: [] }));
    if (row.kind === "delete") seg.dels.push(row);
    else seg.inss.push(row);
  }
  flushRun();
  closeEqual();
  return { blocks, left, right, unified, moves: [...moves.values()] };
}

/** The change keys of every block cut from one fold-block (`Block.hunk`): what a hunk
    header's Select, Stage, Unstage and Discard act on. */
export function hunkKeys(model: BlockModel, hunk: number): Set<string> {
  const keys = new Set<string>();
  for (const b of model.blocks) {
    if (b.hunk !== hunk || b.kind === "equal") continue;
    for (const k of b.keys.deletes) keys.add(k);
    for (const k of b.keys.inserts) keys.add(k);
  }
  return keys;
}

/** What F6 steps between: a run of non-equal blocks with nothing equal between them,
    as block ids (both ends included). */
export interface Change {
  from: number;
  to: number;
}

export function changes(model: BlockModel): Change[] {
  const out: Change[] = [];
  for (const b of model.blocks) {
    if (b.kind === "equal") continue;
    const last = out[out.length - 1];
    if (last && last.to === b.id - 1) last.to = b.id;
    else out.push({ from: b.id, to: b.id });
  }
  return out;
}

/** Where a change starts in the list a jump scrolls: the left pane's rows side by side (an
    empty left side is the point the change sits at), the unified rows otherwise. */
export function changeRow(model: BlockModel, change: Change, unified: boolean): number {
  const b = model.blocks[change.from]!;
  return unified ? b.unifiedRow : b.leftRow;
}
