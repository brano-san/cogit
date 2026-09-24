import { GRAPH } from "$lib/graph-geometry";
import type { GraphOverlay, GraphRow, Segment } from "$lib/ipc";

/** `--graph-branch-1` … `--graph-branch-8`: the colours of branches ticked in Branches. */
export const BRANCH_SLOTS = 8;

/** `graph_engine::PAINT_SLOT`. */
const SLOT_BITS = 0x0f;

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
}

/** How a line or a ring is stroked. Lower layers go first, so what matters is on top. */
export interface Stroke {
  token: string;
  width: number;
  layer: number;
}

export const LAYERS = 3;
const LAYER = { grey: 0, colour: 1, main: 2 } as const;

function stroke(style: number, primary: boolean, laneColour: number, width: number, options: StrokeOptions): Stroke {
  const slot = style & SLOT_BITS;
  const token =
    slot > 0
      ? branchToken(slot - 1)
      : options.colouredLanes
        ? `--c-lane-${(laneColour % BRANCH_SLOTS) + 1}`
        : primary
          ? "--graph-main"
          : "--graph-line";
  return {
    token,
    width: primary ? GRAPH.mainLineWidth : width,
    layer: primary ? LAYER.main : slot > 0 ? LAYER.colour : LAYER.grey,
  };
}

export function segmentStroke(
  segment: Segment,
  index: number,
  paint: RowPaint | undefined,
  options: StrokeOptions,
): Stroke {
  return stroke(paint?.segmentStyles[index] ?? 0, segment.primary, segment.color, GRAPH.lineWidth, options);
}

/** A ring keeps its stroke width; only its colour follows the paint. */
export function nodeStroke(layout: GraphRow, paint: RowPaint | undefined, options: StrokeOptions): Stroke {
  return {
    ...stroke(paint?.nodeStyle ?? 0, layout.primary, layout.color, GRAPH.ringStroke, options),
    width: GRAPH.ringStroke,
  };
}
