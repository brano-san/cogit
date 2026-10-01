import {
  ACTION_LEFT_X,
  ACTION_RIGHT_X,
  curveFraction,
  edgesOf,
  pathOf,
  visibleCenter,
} from "./diff-band";

/** Every line of the three panes is this tall, and none wraps: a pane's geometry is then
    arithmetic on line numbers and padding, with no measuring (and so exact off screen). */
export const LINE_HEIGHT = 18;
/** Rows of context a jump leaves above the hunk it lands on. */
export const LEAD = 3;

export type SolverLayout = "oursResult" | "all" | "resultTheirs" | "resultBelow";

export const LAYOUTS: readonly { id: SolverLayout; label: string }[] = [
  { id: "oursResult", label: "Ours + Result" },
  { id: "all", label: "All" },
  { id: "resultTheirs", label: "Result + Theirs" },
  { id: "resultBelow", label: "Result below" },
];

export function panesOf(layout: SolverLayout): { ours: boolean; theirs: boolean; below: boolean } {
  switch (layout) {
    case "oursResult":
      return { ours: true, theirs: false, below: false };
    case "resultTheirs":
      return { ours: false, theirs: true, below: false };
    case "resultBelow":
      return { ours: true, theirs: true, below: true };
    case "all":
      return { ours: true, theirs: true, below: false };
  }
}

/** Where a hunk sits in one pane, in lines. */
export interface BlockRows {
  start: number;
  count: number;
}

export interface Edge {
  top: number;
  bottom: number;
}

export interface PaneRows {
  ours: number;
  result: number;
  theirs: number;
}

/** In Aligned every hunk is as tall in every pane shown: the shorter ones get filler at the
    end of the hunk. Lines are counted per pane, padding comes out in lines. */
export function alignedPads(
  rows: readonly PaneRows[],
  shown: { ours: boolean; result: boolean; theirs: boolean },
): { ours: number[]; result: number[]; theirs: number[] } {
  const pad = (row: PaneRows, pane: keyof PaneRows): number => {
    if (!shown[pane]) return 0;
    const tallest = Math.max(...(["ours", "result", "theirs"] as const).filter((p) => shown[p]).map((p) => row[p]));
    return tallest - row[pane];
  };
  return {
    ours: rows.map((row) => pad(row, "ours")),
    result: rows.map((row) => pad(row, "result")),
    theirs: rows.map((row) => pad(row, "theirs")),
  };
}

/** The pixel extent of every hunk of a pane, padding at the end of a hunk included. */
export function edgesOfBlocks(blocks: readonly BlockRows[], pads: readonly number[]): Edge[] {
  let padded = 0;
  return blocks.map((block, at) => {
    const top = (block.start + padded) * LINE_HEIGHT;
    padded += pads[at] ?? 0;
    return { top, bottom: (block.start + block.count + padded) * LINE_HEIGHT };
  });
}

export function contentHeight(lines: number, pads: readonly number[]): number {
  return (lines + pads.reduce((sum, pad) => sum + pad, 0)) * LINE_HEIGHT;
}

function breakpoints(edges: readonly Edge[], content: number): number[] {
  return [0, ...edges.flatMap((edge) => [edge.top, edge.bottom]), content];
}

/** The `scrollTop` of the target pane that shows what the source pane shows at `y`: hunks
    map onto each other, everything between them one to one. A source resting on its bottom
    puts the target on its own, so panes of different height end flush. */
export function mapScroll(
  from: readonly Edge[],
  to: readonly Edge[],
  y: number,
  fromContent: number,
  toContent: number,
  viewport: number,
): number {
  const max = Math.max(toContent - viewport, 0);
  const sourceMax = Math.max(fromContent - viewport, 0);
  if (y > 0 && y >= sourceMax) return max;
  const src = breakpoints(from, fromContent);
  const dst = breakpoints(to, toContent);
  let low = 0;
  let high = src.length - 2;
  while (low < high) {
    const mid = (low + high + 1) >> 1;
    if ((src[mid] ?? 0) <= y) low = mid;
    else high = mid - 1;
  }
  const s0 = src[low] ?? 0;
  const s1 = src[low + 1] ?? s0;
  const d0 = dst[low] ?? 0;
  const d1 = dst[low + 1] ?? d0;
  const last = low >= src.length - 2;
  const mapped = last || s1 === s0 ? d0 + (y - s0) : d0 + ((y - s0) * (d1 - d0)) / (s1 - s0);
  return Math.min(Math.max(Math.round(mapped), 0), max);
}

export function scrollToBlock(edge: Edge, viewport: number, content: number): number {
  const max = Math.max(content - viewport, 0);
  return Math.min(Math.max(edge.top - LEAD * LINE_HEIGHT, 0), max);
}

export interface BandConnector {
  id: number;
  left: [number, number];
  right: [number, number];
  path: string;
  edges: string;
  /** Where the button goes in the gutter: » in the left part, « in the right part. */
  anchor: { left: number | null; right: number | null };
}

const LEFT_FRACTION = curveFraction(ACTION_LEFT_X);
const RIGHT_FRACTION = curveFraction(ACTION_RIGHT_X);

/** Ribbons for the hunks near the viewport, in the band's own y axis. `ids[i]` names the
    hunk of `left[i]` and `right[i]`. */
export function bandConnectors(
  left: readonly Edge[],
  right: readonly Edge[],
  leftScroll: number,
  rightScroll: number,
  height: number,
  ids: readonly number[],
  inset: number,
): BandConnector[] {
  const out: BandConnector[] = [];
  left.forEach((l, at) => {
    const r = right[at];
    const id = ids[at];
    if (!r || id === undefined) return;
    const a = Math.round(l.top - leftScroll);
    const b = Math.round(l.bottom - leftScroll);
    const x = Math.round(r.top - rightScroll);
    const y = Math.round(r.bottom - rightScroll);
    if (Math.min(a, x) > height || Math.max(b, y) < 0) return;
    const centre = (f: number) => visibleCenter(a + (x - a) * f, b + (y - b) * f, height, inset);
    out.push({
      id,
      left: [a, b],
      right: [x, y],
      path: pathOf(a, b, x, y),
      edges: edgesOf(a, b, x, y),
      anchor: { left: centre(LEFT_FRACTION), right: centre(RIGHT_FRACTION) },
    });
  });
  return out;
}
