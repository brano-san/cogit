import { describe, expect, it } from "vitest";
import { bisectLooks, dotToken, rowToken } from "./bisect-looks";
import { nodeFill } from "./graph-geometry";
import type { BisectState } from "./ipc/bisect";

const A = "a".repeat(40);
const B = "b".repeat(40);
const C = "c".repeat(40);
const D = "d".repeat(40);

function state(over: Partial<BisectState> = {}): BisectState {
  return {
    start: "main",
    bad: A,
    good: [B],
    skipped: [],
    current: C,
    firstBad: null,
    candidates: [],
    terms: { bad: "bad", good: "good" },
    ...over,
  };
}

describe("bisectLooks", () => {
  it("has nothing to show outside a bisect", () => {
    expect(bisectLooks(null).size).toBe(0);
  });

  it("dots good commits green, the bad one red and skipped ones grey", () => {
    const looks = bisectLooks(state({ skipped: [D] }));
    expect(dotToken(looks.get(A))).toBe("--graph-bisect-bad");
    expect(dotToken(looks.get(B))).toBe("--graph-bisect-good");
    expect(dotToken(looks.get(D))).toBe("--graph-bisect-skip");
    expect(looks.get(D)?.tag).toBe("skipped");
  });

  it("tints the row of the commit under test and says so", () => {
    const current = bisectLooks(state()).get(C);
    expect(rowToken(current)).toBe("--graph-bisect-current");
    expect(dotToken(current)).toBeNull();
    expect(current?.tag).toBe("testing");
  });

  it("keeps the mark of a HEAD that has one", () => {
    const head = bisectLooks(state({ good: [], current: A })).get(A);
    expect(dotToken(head)).toBe("--graph-bisect-bad");
    expect(rowToken(head)).toBe("--graph-bisect-current");
    expect(head?.tag).toBe("bad");
  });

  it("marks the first bad commit at the end and nothing as under test", () => {
    const looks = bisectLooks(state({ firstBad: A, current: A }));
    expect(rowToken(looks.get(A))).toBe("--graph-bisect-found");
    expect(looks.get(A)?.tag).toBe("first bad");
    expect([...looks.values()].some((look) => look.row === "current")).toBe(false);
  });

  it("tints every candidate when only skipped commits are left", () => {
    const looks = bisectLooks(state({ skipped: [C], candidates: [C, A] }));
    expect(rowToken(looks.get(C))).toBe("--graph-bisect-found");
    expect(rowToken(looks.get(A))).toBe("--graph-bisect-found");
    expect(dotToken(looks.get(C))).toBe("--graph-bisect-skip");
    expect(looks.get(C)?.tag).toBe("candidate");
  });

  it("names the marks in the words of the bisect", () => {
    const looks = bisectLooks(state({ terms: { bad: "broken", good: "fixed" }, firstBad: A }));
    expect(looks.get(B)?.tag).toBe("fixed");
    expect(looks.get(A)?.tag).toBe("first broken");
  });
});

describe("the fill of a node on a tinted row", () => {
  // The row's tint replaces its stripe, as the CSS of the row does; hover and selection
  // still cover it.
  it("is the tint in place of the stripe, under hover and selection", () => {
    expect(nodeFill(3, [], null, true, "--graph-bisect-current")).toEqual([
      "--surface-panel",
      "--graph-bisect-current",
    ]);
    expect(nodeFill(3, [3], 3, true, "--graph-bisect-found")).toEqual([
      "--surface-panel",
      "--graph-bisect-found",
      "--state-hover",
      "--state-selected",
    ]);
    expect(nodeFill(3, [], null, true, null)).toEqual(["--surface-panel", "--row-stripe"]);
  });
});
