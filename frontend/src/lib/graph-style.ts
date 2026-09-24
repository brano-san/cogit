import { GRAPH } from "$lib/graph-geometry";
import type { GraphOverlay, GraphRow, Segment } from "$lib/ipc";

/** `--graph-branch-1` … `--graph-branch-8`: the colours of branches ticked in Branches. */
export const BRANCH_SLOTS = 8;

/** `graph_engine::PAINT_SLOT`. */
const SLOT_BITS = 0x0f;
/** The branch of the chosen commit stands out from the main line too. */
export const FOCUS_LINE_WIDTH = GRAPH.mainLineWidth + 1;

/** FNV-1a over the UTF-16 units: the same branch gets the same colour in every run and on
    every machine, whatever else is ticked. */
export function branchSlot(name: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < name.length; i++) {
    hash ^= name.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % BRANCH_SLOTS;
}

export function branchToken(slot: number): string {
  return `--graph-branch-${(slot % BRANCH_SLOTS) + 1}`;
}

/** The paint of one row, as `graph_overlay` cut it. */
export interface RowPaint {
  nodeLane: number;
  nodeStyle: number;
  segmentLanes: readonly number[];
  segmentStyles: readonly number[];
}

export function rowPaint(overlay: GraphOverlay, row: number): RowPaint | undefined {
  const at = row - overlay.start;
  if (at < 0 || at >= overlay.nodeStyles.length) return undefined;
  const from = overlay.segmentFirst[at] ?? 0;
  const to = overlay.segmentFirst[at + 1] ?? from;
  return {
    nodeLane: overlay.nodeLanes[at] ?? -1,
    nodeStyle: overlay.nodeStyles[at] ?? 0,
    segmentLanes: overlay.segmentLanes.slice(from, to),
    segmentStyles: overlay.segmentStyles.slice(from, to),
  };
}

export interface StrokeOptions {
  /** The `Coloured branch lines` setting: a colour per lane where no branch has one. */
  colouredLanes: boolean;
  /** Drawn on top and wider: the branch of the chosen commit (#26). */
  focusLane?: number | null;
}

/** How a line or a ring is stroked. Lower layers go first, so what matters is on top. */
export interface Stroke {
  token: string;
  width: number;
  layer: number;
}

export const LAYERS = 4;
const LAYER = { grey: 0, colour: 1, main: 2, focus: 3 } as const;

function stroke(
  style: number,
  lane: number | undefined,
  primary: boolean,
  laneColour: number,
  width: number,
  options: StrokeOptions,
): Stroke {
  const slot = style & SLOT_BITS;
  const focused = options.focusLane != null && lane === options.focusLane;
  const token =
    slot > 0
      ? branchToken(slot - 1)
      : focused && !primary
        ? "--graph-focus"
        : options.colouredLanes
          ? `--c-lane-${(laneColour % BRANCH_SLOTS) + 1}`
          : primary
            ? "--graph-main"
            : "--graph-line";
  return {
    token,
    width: focused ? FOCUS_LINE_WIDTH : primary ? GRAPH.mainLineWidth : width,
    layer: focused ? LAYER.focus : primary ? LAYER.main : slot > 0 ? LAYER.colour : LAYER.grey,
  };
}

export function segmentStroke(
  segment: Segment,
  index: number,
  paint: RowPaint | undefined,
  options: StrokeOptions,
): Stroke {
  return stroke(
    paint?.segmentStyles[index] ?? 0,
    paint?.segmentLanes[index],
    segment.primary,
    segment.color,
    GRAPH.lineWidth,
    options,
  );
}

/** A ring keeps its stroke width; only its colour follows the paint. */
export function nodeStroke(layout: GraphRow, paint: RowPaint | undefined, options: StrokeOptions): Stroke {
  return {
    ...stroke(paint?.nodeStyle ?? 0, paint?.nodeLane, layout.primary, layout.color, GRAPH.ringStroke, options),
    width: GRAPH.ringStroke,
  };
}

/** The lane of the line at `column` in one half of a row, for a click on a line rather
    than on a ring; `null` where no line runs. */
export function laneAt(
  layout: GraphRow,
  paint: RowPaint | undefined,
  column: number,
  upperHalf: boolean,
): number | null {
  if (!paint) return null;
  if (column === layout.lane) return paint.nodeLane;
  for (const [index, segment] of layout.segments.entries()) {
    if (segment.arrow || segment.span === (upperHalf ? "bottom" : "top")) continue;
    if (segment.from === column || segment.to === column) return paint.segmentLanes[index] ?? null;
  }
  return null;
}
