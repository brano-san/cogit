import { alignLines } from "./diff-align";
import type { BlockModel, PaneFiller, PaneLine, PaneRow } from "./diff-blocks";

/**
 * The Aligned layout (08 §12.2): the same blocks as the compact model, arranged so that the
 * two panes always have the same rows. Where one side has no line the row is filler. Every
 * block then starts at the same row on both sides and is as tall on both, so the existing
 * machinery needs no special case: scroll sync is the identity, a connector is a straight band,
 * a fold is one row in both panes.
 *
 * - equal: line against line (a fold: one row each).
 * - added / removed / moved halves: the lines, with as many filler rows opposite.
 * - changed: lines paired by similarity (`alignLines`), the rest opposite filler.
 *
 * Unified rows, keys and inline ranges are shared with the compact model, not rebuilt.
 */
export function alignModel(model: BlockModel): BlockModel {
  const left: PaneRow[] = [];
  const right: PaneRow[] = [];

  const blocks = model.blocks.map((b) => {
    const row = left.length;
    const l = model.left.slice(b.leftRow, b.leftRow + b.leftRows);
    const r = model.right.slice(b.rightRow, b.rightRow + b.rightRows);
    const filler = (): PaneFiller => ({ kind: "filler", block: b.id, blockKind: b.kind });
    const push = (x: PaneRow | undefined, y: PaneRow | undefined) => {
      left.push(x ?? filler());
      right.push(y ?? filler());
    };

    if (b.kind === "changed") {
      const text = (x: PaneRow) => (x as PaneLine).text;
      for (const [i, j] of alignLines(l.map(text), r.map(text))) push(i === null ? undefined : l[i], j === null ? undefined : r[j]);
    } else {
      // equal (the sides are as long as each other), added, removed, one half of a move.
      for (let k = 0; k < Math.max(l.length, r.length); k++) push(l[k], r[k]);
    }
    const rows = left.length - row;
    return { ...b, leftRow: row, leftRows: rows, rightRow: row, rightRows: rows };
  });
  return { ...model, blocks, left, right };
}
