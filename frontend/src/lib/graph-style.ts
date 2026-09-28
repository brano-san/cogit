import { GRAPH } from "$lib/graph-geometry";
import type { GraphOverlay, GraphRow, Segment } from "$lib/ipc";

/** `--graph-branch-1` … `--graph-branch-8`: the colours of branches ticked in Branches. */
export const BRANCH_SLOTS = 8;

/** `graph_engine::PAINT_SLOT` and `PAINT_DIM`. */
const SLOT_BITS = 0x0f;
const DIM_BIT = 0x10;
/** Outside the ancestry of the chosen commit (#26) lines and rings are this faint. */
export const DIM_ALPHA = 0.28;
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
  /** Branch Coloring: all but `focusLane`, or the main line without one, is dimmed. */
  branchOnly?: boolean;
  /** Mergeable Coloring: what is not dimmed is drawn in the accent. */
  accentLit?: boolean;
  /** Drawn on top and wider: the branch of the chosen commit (#26). */
  focusLane?: number | null;
}

/** How a line or a ring is stroked. Lower layers go first, so what matters is on top. */
export interface Stroke {
  token: string;
  width: number;
  alpha: number;
  layer: number;
}

export const LAYERS = 5;
const LAYER = { dim: 0, grey: 1, colour: 2, main: 3, focus: 4 } as const;

function stroke(
  painted: number | undefined,
  lane: number | undefined,
  primary: boolean,
  width: number,
  options: StrokeOptions,
): Stroke {
  // Mergeable dims nearly all: a row whose paint is still on its way waits dimmed, not lit.
  const style = painted ?? (options.accentLit ? DIM_BIT : 0);
  const slot = style & SLOT_BITS;
  const focused = options.focusLane != null && lane === options.focusLane;
  const outside = options.branchOnly === true && (options.focusLane != null ? !focused : !primary);
  const dim = (style & DIM_BIT) !== 0 || outside;
  const token =
    slot > 0
      ? branchToken(slot - 1)
      : focused && !primary
        ? "--graph-focus"
        : options.accentLit && !dim && !primary
          ? "--status-add"
          : primary
            ? "--graph-main"
            : "--graph-line";
  return {
    token,
    width: focused ? FOCUS_LINE_WIDTH : primary ? GRAPH.mainLineWidth : width,
    alpha: dim ? DIM_ALPHA : 1,
    layer: dim
      ? LAYER.dim
      : focused
        ? LAYER.focus
        : primary
          ? LAYER.main
          : slot > 0
            ? LAYER.colour
            : LAYER.grey,
  };
}

export function segmentStroke(
  segment: Segment,
  index: number,
  paint: RowPaint | undefined,
  options: StrokeOptions,
): Stroke {
  return stroke(
    paint?.segmentStyles[index] ?? (paint ? 0 : undefined),
    paint?.segmentLanes[index],
    segment.primary,
    GRAPH.lineWidth,
    options,
  );
}

/** A ring keeps its stroke width; only its colour and faintness follow the paint. */
export function nodeStroke(layout: GraphRow, paint: RowPaint | undefined, options: StrokeOptions): Stroke {
  return {
    ...stroke(paint?.nodeStyle, paint?.nodeLane, layout.primary, GRAPH.ringStroke, options),
    width: GRAPH.ringStroke,
  };
}

/** A faint line mixed into the background and drawn opaque: overlapping round caps of a
    translucent stroke add up into bright dots at every join. */
export function opaqueInk(colour: string, alpha: number, background: string): string {
  return alpha >= 1 ? colour : `color-mix(in srgb, ${colour} ${Math.round(alpha * 100)}%, ${background})`;
}

/** A commit whose ring is dimmed has its text grayed too. */
export function rowFaded(layout: GraphRow, paint: RowPaint | undefined, options: StrokeOptions): boolean {
  // Text does not wait dimmed with its ring: unpainted, it stays as it is until the paint says.
  return nodeStroke(layout, paint, paint ? options : { ...options, accentLit: false }).alpha < 1;
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
