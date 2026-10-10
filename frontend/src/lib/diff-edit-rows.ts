import type { BlockKind, PaneRow, UnifiedLine, UnifiedRow } from "./diff-blocks";
import type { Gap } from "./diff-fold";
import type { EditRow, Mark, Pick, Tone } from "./diff-pane-editor";
import { mergePieces, type Token } from "./highlight";

/** Colors go by meaning: red is what is gone, green what is new, violet what moved. Side by
    side (`joint`) a changed block is orange on both sides, every line of it; unified keeps its
    lines apart, red and green. */
export function toneOf(kind: BlockKind, deleting: boolean, joint = false): Tone {
  if (kind === "equal") return "";
  if (kind === "moved") return "move";
  return kind === "changed" && joint ? "changed" : deleting ? "del" : "add";
}

/** How the rows of the editable pane look besides their text: the diff view's own state. */
export interface RowLook {
  selected: ReadonlySet<string>;
  stageable: boolean;
  flash: { from: number; to: number } | null;
  /** A row's search hits; `current` tells the hit the counter points at by its start. */
  hits: (index: number) => readonly [number, number][];
  current: (index: number, start: number) => boolean;
  /** The strip a fold draws, and what tells two strips apart. */
  fold: (gap: Gap, index: number) => { id: string; render: (host: HTMLElement) => () => void };
}

function pickOf(key: string | null, look: RowLook): Pick {
  return key !== null && look.selected.has(key) ? (look.stageable ? "staging" : "marked") : null;
}

function flashed(row: { block: number; blockKind: BlockKind }, look: RowLook): boolean {
  const flash = look.flash;
  return flash !== null && row.blockKind !== "equal" && row.block >= flash.from && row.block <= flash.to;
}

function marksOf(inline: readonly [number, number][], index: number, look: RowLook): Mark[] {
  const marks: Mark[] = inline.map(([from, to]) => ({ from, to, cls: "word" }));
  for (const [from, to] of look.hits(index)) {
    marks.push({ from, to, cls: look.current(index, from) ? "hit current" : "hit" });
  }
  return marks;
}

function foldOf(gap: Gap, index: number, look: RowLook): EditRow {
  return { kind: "fold", from: gap.newFrom, hidden: gap.hidden, ...look.fold(gap, index) };
}

/** Side by side: the right pane's rows, as lines of the file, folds and filler. */
export function splitRows(rows: readonly PaneRow[], look: RowLook): EditRow[] {
  return rows.map((row, index): EditRow => {
    if (row.kind === "gap") return foldOf(row.gap, index, look);
    if (row.kind === "filler") return { kind: "filler" };
    return {
      kind: "code",
      line: row.line,
      tone: toneOf(row.blockKind, false, true),
      old: null,
      key: row.key,
      pick: pickOf(row.key, look),
      flash: flashed(row, look),
      marks: marksOf(row.inline, index, look),
    };
  });
}

/** Unified: the lines of the file, with the deleted ones between them. A deleted line is not
    in the file, so it brings its own highlighting (`tokens`). */
export function unifiedRows(
  rows: readonly UnifiedRow[],
  look: RowLook,
  tokens: (row: UnifiedLine) => readonly Token[],
): EditRow[] {
  return rows.map((row, index): EditRow => {
    if (row.kind === "gap") return foldOf(row.gap, index, look);
    const common = { key: row.key, pick: pickOf(row.key, look), flash: flashed(row, look) };
    if (row.type === "delete") {
      const pieces = mergePieces(row.text, tokens(row), row.inline, look.hits(index)).map((piece) => ({
        text: piece.text,
        cls: [piece.cls, piece.changed && "word", piece.hit && "hit", piece.hit && look.current(index, piece.start) && "current"]
          .filter(Boolean)
          .join(" "),
      }));
      return { kind: "deleted", old: row.old ?? 0, tone: toneOf(row.blockKind, true), pieces, ...common };
    }
    return {
      kind: "code",
      line: row.new ?? 0,
      tone: toneOf(row.blockKind, false),
      old: row.type === "context" ? row.old : null,
      marks: marksOf(row.inline, index, look),
      ...common,
    };
  });
}
