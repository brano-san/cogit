import { describe, expect, it } from "vitest";
import { GRAPH, arrowStub, laneX, nodeCentre } from "./graph-geometry";
import { linkStubs, linkTitle } from "./graph-links";
import type { Segment } from "./ipc";

const arrow = (from: number, to: number, span: Segment["span"]): Segment => ({
  from,
  to,
  span,
  primary: false,
  color: 0,
  arrow: true,
});

describe("the parent's end of a cut link", () => {
  it("comes down into the ring from the upper right, inside its own row", () => {
    const stub = arrowStub(arrow(1, 2, "top"), 4, 0);
    const centre = nodeCentre(1, 4, 0);

    expect(stub.x2).toBeGreaterThan(centre.x);
    expect(stub.x1).toBeGreaterThan(stub.x2);
    expect(stub.y1).toBeLessThan(stub.y2);
    expect(stub.y2).toBeLessThan(centre.y);
    expect(stub.y1).toBeGreaterThanOrEqual(4 * GRAPH.rowHeight);
    expect(stub.x1).toBeLessThan(laneX(2) - GRAPH.laneWidth / 2);
  });

  it("has its head at the ring, pointing in", () => {
    const stub = arrowStub(arrow(1, 2, "top"), 4, 0);
    const centre = nodeCentre(1, 4, 0);
    const tip = Math.hypot(stub.x2 - centre.x, stub.y2 - centre.y);

    for (const barb of [stub.left, stub.right]) {
      expect(Math.hypot(barb.x - centre.x, barb.y - centre.y)).toBeGreaterThan(tip);
    }
  });

  it("leaves the child's end as it was: pointing away, down", () => {
    expect(arrowStub(arrow(1, 1, "bottom"), 4, 0)).toEqual(arrowStub({ from: 1, to: 1 }, 4, 0));
  });
});

describe("linkStubs", () => {
  it("gives each stub a target over its arrow, with every commit at the far end", () => {
    const layout = {
      segments: [
        { ...arrow(0, 0, "through"), arrow: false },
        arrow(1, 2, "top"),
        arrow(1, 1, "bottom"),
      ],
      links: [
        { segment: 1, oid: "near" },
        { segment: 1, oid: "far" },
        { segment: 2, oid: "parent" },
      ],
    };

    const stubs = linkStubs(layout);

    expect(stubs.map((stub) => [stub.segment, stub.oids])).toEqual([
      [1, ["near", "far"]],
      [2, ["parent"]],
    ]);
    const down = arrowStub(layout.segments[2]!, 0, 0);
    const box = stubs[1]!.box;
    expect(box.left).toBeLessThan(down.x1);
    expect(box.left + box.size).toBeGreaterThan(down.x1);
    expect(box.top).toBeLessThan(down.y2);
    expect(box.top + box.size).toBeGreaterThan(down.y1);
  });

  it("has nothing to point at in a row without cut links", () => {
    expect(linkStubs({ segments: [arrow(0, 0, "bottom")], links: [] })).toEqual([]);
  });
});

describe("linkTitle", () => {
  const describe_ = (oid: string) => ({ short: oid.slice(0, 7), summary: oid === "bbbbbbbbbb" ? null : `subject of ${oid}` });

  it("names the far end by its short hash and subject", () => {
    expect(linkTitle(["aaaaaaaaaa"], describe_)).toBe("aaaaaaa subject of aaaaaaaaaa");
  });

  it("names a commit not loaded yet by its hash alone", () => {
    expect(linkTitle(["bbbbbbbbbb"], describe_)).toBe("bbbbbbb");
  });

  it("counts the rest past a handful", () => {
    const oids = Array.from({ length: 9 }, (_, i) => `${i}`.repeat(10));
    const lines = linkTitle(oids, describe_).split("\n");

    expect(lines).toHaveLength(7);
    expect(lines.at(-1)).toBe("and 3 more");
  });
});
