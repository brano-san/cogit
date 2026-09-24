import { arrowStub } from "$lib/graph-geometry";
import type { GraphRow } from "$lib/ipc";

/** The stubs of cut links in one row (R-330), as things to point at: one per arrow, with
    every commit at its far end, the nearest first. */
export interface LinkStub {
  segment: number;
  oids: string[];
  /** Relative to the row's top left corner, CSS pixels. */
  box: { left: number; top: number; size: number };
}

/** Bigger than the arrow: six pixels of line are hard to hit. */
const HIT = 12;

export function linkStubs(layout: Pick<GraphRow, "segments" | "links">): LinkStub[] {
  const stubs = new Map<number, LinkStub>();
  for (const link of layout.links) {
    const segment = layout.segments[link.segment];
    if (!segment) continue;
    let stub = stubs.get(link.segment);
    if (!stub) {
      const arrow = arrowStub(segment, 0, 0);
      const x = (arrow.x1 + arrow.x2) / 2;
      const y = (arrow.y1 + arrow.y2) / 2;
      stub = { segment: link.segment, oids: [], box: { left: x - HIT / 2, top: y - HIT / 2, size: HIT } };
      stubs.set(link.segment, stub);
    }
    stub.oids.push(link.oid);
  }
  return [...stubs.values()];
}

/** How many far ends one tooltip names before it counts the rest. */
const NAMED = 6;

/** `a1b2c3d Fix the thing`, one line per far end; a commit not loaded yet is its hash. */
export function linkTitle(
  oids: readonly string[],
  describe: (oid: string) => { short: string; summary: string | null },
): string {
  const lines = oids.slice(0, NAMED).map((oid) => {
    const { short, summary } = describe(oid);
    return summary ? `${short} ${summary}` : short;
  });
  if (oids.length > NAMED) lines.push(`and ${oids.length - NAMED} more`);
  return lines.join("\n");
}
