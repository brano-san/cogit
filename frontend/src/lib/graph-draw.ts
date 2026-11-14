import {
  CSS_GRID,
  GRAPH,
  nodeSizes,
  snapToGrid,
  type PixelGrid,
  arrowStub,
  laneX,
  nodeCentre,
  nodeFillInto,
  rowY,
  segmentCurve,
  textX,
  throughWorkingTree,
  transition,
  type Curve,
} from "$lib/graph-geometry";
import { LAYERS, nodeStroke, opaqueInk, segmentStroke, type RowPaint } from "$lib/graph-style";
import type { GraphColoring } from "$lib/graph-coloring";
import type { GraphRow, Segment } from "$lib/ipc";

/** A bisect mark (F-566): the dot fills the ring, the tint is the row behind it. Tokens. */
export interface BisectPaint {
  dot: string | null;
  tint: string | null;
}

/** One row on the canvas. `unpushed`: a dot with a gap around it; a commit some remote
    branch has is a hollow ring. */
export interface DrawnRow {
  listRow: number;
  layout: GraphRow;
  stash?: boolean;
  unpushed?: boolean;
  paint?: RowPaint;
  bisect?: BisectPaint;
}

/** What one frame of the graph shows; `GraphCanvas` documents each field. */
export interface GraphScene {
  rows: readonly DrawnRow[];
  scrollTop: number;
  height: number;
  headRow: number | null;
  headLane: number | null;
  headerRow: number;
  headerLane: number | null;
  workingTree: "clean" | "changed" | null;
  workingTreeLayout: GraphRow | null;
  selectedRows: readonly number[];
  hoverRow: number | null;
  focusLane: number | null;
  clipX: number;
  stripes: boolean;
  coloring: GraphColoring;
  /** The device pixels the canvas is drawn on; nodes snap to them. CSS pixels when absent. */
  grid?: PixelGrid;
}

/** One path per look in a layer, as the frame fills it. */
interface Bucket {
  style: string;
  width: number;
  path: Path2D;
}

/** Kept between frames, so a frame makes no maps, arrays or curves of its own (doc/15 §10). */
const scratch = {
  layers: Array.from({ length: LAYERS }, (): Bucket[] => []),
  segmentsAt: new Map<number, readonly Segment[]>(),
  curve: { x1: 0, y1: 0, cx1: 0, cy1: 0, cx2: 0, cy2: 0, x2: 0, y2: 0 } as Curve,
  near: {} as { above?: readonly Segment[]; below?: readonly Segment[] },
  behind: [] as string[],
};

/** `opaqueInk` per colour and faintness for one panel colour: a dimmed line otherwise built a
    `color-mix()` string per segment per frame. Another theme brings another panel colour. */
let inkPanel = "";
const inks = new Map<number, Map<string, string>>();
function inkOf(colour: string, alpha: number, panel: string): string {
  if (alpha >= 1) return colour;
  if (panel !== inkPanel) {
    inks.clear();
    inkPanel = panel;
  }
  let byColour = inks.get(alpha);
  if (!byColour) {
    byColour = new Map();
    inks.set(alpha, byColour);
  }
  let ink = byColour.get(colour);
  if (ink === undefined) {
    ink = opaqueInk(colour, alpha, panel);
    byColour.set(colour, ink);
  }
  return ink;
}

const DASH = [3, 3];

/** Paints `scene` on a context already scaled to CSS pixels and cleared. `token` resolves a
    CSS custom property to a colour, and is asked on every frame: it caches (`tokenColor`). */
export function paintGraph(context: CanvasRenderingContext2D, scene: GraphScene, token: (name: string) => string): void {
  const {
    rows,
    scrollTop,
    height,
    headRow,
    headLane,
    headerRow,
    headerLane,
    workingTree,
    workingTreeLayout,
    selectedRows,
    hoverRow,
    focusLane,
    clipX,
    stripes,
    coloring,
    grid = CSS_GRID,
  } = scene;
  const sizes = nodeSizes(grid.ratio);
  const main = token("--graph-main");
  const line = token("--graph-line");
  const options = { accentLit: coloring === "mergeable", branchOnly: coloring === "branch", focusLane };
  const panel = token("--bg-editor");
  const ink = (name: string, alpha: number) => inkOf(token(name) || line, alpha, panel);

  const edge = Math.min(textX(GRAPH.maxColumns), clipX) - GRAPH.textGap;
  context.save();
  context.beginPath();
  context.rect(0, 0, edge, height);
  context.clip();

  // A merge in progress: its Working Tree row is drawn after the rows, as one of them.
  const count = rows.length + (workingTreeLayout ? 1 : 0);
  const wtRow: DrawnRow | null = workingTreeLayout ? { listRow: headerRow, layout: workingTreeLayout } : null;
  const { layers, segmentsAt, curve: into, near } = scratch;
  segmentsAt.clear();
  for (let at = 0; at < count; at++) {
    const row = at < rows.length ? rows[at]! : wtRow!;
    segmentsAt.set(row.listRow, row.layout.segments);
  }
  // Grey first, branch colours over it, the main line on top: where lines cross, the one
  // the eye follows wins. One path per look in a layer: a line's segments meet at every row
  // edge, and stroked one by one their round caps were painted twice there, a tick across
  // the line on every row. One pass sorts every segment into its layer's paths.
  for (let at = 0; at < count; at++) {
    const row = at < rows.length ? rows[at]! : wtRow!;
    const segments = row.layout.segments;
    near.above = segmentsAt.get(row.listRow - 1);
    near.below = segmentsAt.get(row.listRow + 1);
    for (let index = 0; index < segments.length; index++) {
      const segment = segments[index]!;
      const look = segmentStroke(segment, index, row.paint, options);
      const style = ink(look.token, look.alpha);
      const buckets = layers[look.layer]!;
      let bucket: Bucket | undefined;
      for (const known of buckets) {
        if (known.style === style && known.width === look.width) {
          bucket = known;
          break;
        }
      }
      if (!bucket) {
        bucket = { style, width: look.width, path: new Path2D() };
        buckets.push(bucket);
      }
      const { path } = bucket;
      const curve = segment.arrow ? null : segmentCurve(segment, row.listRow, scrollTop, near, into);
      // Across the Working Tree row to where the line in HEAD's row starts.
      if (curve && !workingTreeLayout && headerRow > 0 && row.listRow === headRow && segment.span !== "bottom") {
        const x = laneX(segment.from);
        path.moveTo(x, throughWorkingTree(headerRow, scrollTop, segmentsAt.get(headerRow - 1), segment.from));
        path.lineTo(x, Math.min(curve.y1, curve.y2));
      }
      if (segment.arrow) {
        const stub = arrowStub(segment, row.listRow, scrollTop);
        path.moveTo(stub.x1, stub.y1);
        path.lineTo(stub.x2, stub.y2);
        path.moveTo(stub.left.x, stub.left.y);
        path.lineTo(stub.x2, stub.y2);
        path.lineTo(stub.right.x, stub.right.y);
      } else if (curve) {
        path.moveTo(curve.x1, curve.y1);
        if (curve.x1 === curve.x2) path.lineTo(curve.x2, curve.y2);
        else path.bezierCurveTo(curve.cx1, curve.cy1, curve.cx2, curve.cy2, curve.x2, curve.y2);
      }
    }
  }
  near.above = undefined;
  near.below = undefined;
  segmentsAt.clear();
  for (const buckets of layers) {
    for (const { style, width, path } of buckets) {
      context.strokeStyle = style;
      context.lineWidth = width;
      context.stroke(path);
    }
    // The scratch keeps no path, nor the rows it was drawn from, past the frame.
    buckets.length = 0;
  }

  if (!workingTreeLayout && headLane !== null && headRow !== null && scrollTop < GRAPH.rowHeight * headRow) {
    const top = nodeCentre(headerLane ?? headLane, headerRow, scrollTop);
    const foot = nodeCentre(headLane, headRow, scrollTop);
    context.save();
    context.setLineDash(DASH);
    context.strokeStyle = main;
    context.lineWidth = GRAPH.lineWidth;
    context.beginPath();
    context.moveTo(top.x, top.y);
    if (top.x !== foot.x) {
      const turn = transition(top, foot.x, "down");
      context.bezierCurveTo(turn.cx1, turn.cy1, turn.cx2, turn.cy2, turn.x2, turn.y2);
    }
    context.lineTo(foot.x, foot.y);
    context.stroke();
    context.restore();
  }
  // Over the dashed line's end, so the line starts at the dot's edge.
  if (workingTree !== null) {
    const { x, y } = nodeCentre(headerLane ?? headLane ?? 0, headerRow, scrollTop);
    context.beginPath();
    context.arc(snapToGrid(x, grid.ratio, grid.x), snapToGrid(y, grid.ratio, grid.y), sizes.dotRadius, 0, Math.PI * 2);
    context.fillStyle = token(workingTree === "clean" ? "--graph-working-tree-clean" : "--graph-working-tree-changed");
    context.fill();
  }

  // A commit no remote has is a dot in its own colour, the lines meeting its edge as at any
  // other ring (#5); a pushed one is a hollow ring, filled with what is behind it so no line shows through.
  // A stash is a square in the stash colour (#19).
  const stash = token("--status-stash");
  const behind = scratch.behind;
  let wide = false;
  for (const row of rows) {
    const local = !row.stash && row.unpushed === true;
    nodeFillInto(behind, row.listRow, selectedRows, hoverRow, stripes, row.bisect?.tint ?? null);
    // `nodeCentre` and `nodeSquare`, without their objects, on a device-pixel corner.
    const x = snapToGrid(laneX(row.layout.lane), grid.ratio, grid.x);
    const y = snapToGrid(rowY(row.listRow, scrollTop), grid.ratio, grid.y);
    context.beginPath();
    if (row.stash) {
      const size = sizes.ringRadius * 2;
      context.rect(x - size / 2, y - size / 2, size, size);
    } else {
      context.arc(x, y, sizes.ringRadius, 0, Math.PI * 2);
    }
    const ring = nodeStroke(row.layout, row.paint, options);
    const ringInk = row.stash ? stash : ink(ring.token, ring.alpha);
    const dot = row.bisect?.dot ?? null;
    for (const layer of behind) {
      context.fillStyle = token(layer);
      context.fill();
    }
    if (local) {
      context.fillStyle = ringInk;
      context.fill();
    }
    if (dot) {
      context.fillStyle = token(dot);
      context.fill();
    }
    context.lineWidth = sizes.ringStroke;
    context.strokeStyle = ringInk;
    context.stroke();
    // A row wider than the cap or the room left is cut off with a fade, not a hard edge.
    if (laneX(row.layout.width - 1) + GRAPH.laneWidth / 2 > edge) wide = true;
  }
  behind.length = 0;
  context.restore();

  if (wide) {
    const fade = context.createLinearGradient(edge - GRAPH.laneWidth, 0, edge, 0);
    fade.addColorStop(0, "transparent");
    fade.addColorStop(1, panel);
    context.fillStyle = fade;
    context.fillRect(edge - GRAPH.laneWidth, 0, GRAPH.laneWidth, height);
  }
}
