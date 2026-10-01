import { describe, expect, it } from "vitest";
import { SELECTION_CAP, firstOids, oidsBetween, type RowSource } from "./graph-selection";

function rows(count: number, missing: string[] = []): RowSource {
  const oid = (i: number) => `c${i}`;
  return {
    loadedIndexOf: (id) => (id !== null && /^c\d+$/.test(id) && !missing.includes(id) ? Number(id.slice(1)) : null),
    indexOf: async () => null,
    entry: async (i) => (i >= 0 && i < count ? { commit: { oid: oid(i) } } : undefined),
  };
}

describe("oidsBetween", () => {
  it("lists the commits between two, ends included, in display order either way", async () => {
    expect(await oidsBetween(rows(10), "c2", "c5")).toEqual(["c2", "c3", "c4", "c5"]);
    expect(await oidsBetween(rows(10), "c5", "c2")).toEqual(["c2", "c3", "c4", "c5"]);
  });

  it("is one commit when both ends are the same", async () => {
    expect(await oidsBetween(rows(10), "c4", "c4")).toEqual(["c4"]);
  });

  it("is null when an end is not in the graph", async () => {
    expect(await oidsBetween(rows(10, ["c2"]), "c2", "c5")).toBeNull();
  });

  it("asks Rust for the row of a commit whose block is gone", async () => {
    const source: RowSource = { ...rows(10, ["c2"]), indexOf: async () => 2 };
    expect(await oidsBetween(source, "c2", "c4")).toEqual(["c2", "c3", "c4"]);
  });

  it("cuts a range longer than the cap, keeping the end nearest the click", async () => {
    const long = rows(SELECTION_CAP + 500);
    const down = await oidsBetween(long, "c0", `c${SELECTION_CAP + 100}`);
    expect(down).toHaveLength(SELECTION_CAP);
    expect(down?.at(-1)).toBe(`c${SELECTION_CAP + 100}`);
    const up = await oidsBetween(long, `c${SELECTION_CAP + 100}`, "c0");
    expect(up).toHaveLength(SELECTION_CAP);
    expect(up?.[0]).toBe("c0");
  });
});

describe("firstOids", () => {
  it("takes every row of a short graph and the cap of a long one", async () => {
    expect(await firstOids(rows(3), 3)).toEqual(["c0", "c1", "c2"]);
    expect(await firstOids(rows(SELECTION_CAP + 10), SELECTION_CAP + 10)).toHaveLength(SELECTION_CAP);
  });

  it("is empty for an empty graph", async () => {
    expect(await firstOids(rows(0), 0)).toEqual([]);
  });
});
