import { describe, expect, it } from "vitest";
import { affected, clear, mark } from "./staleness";
import type { PanelId } from "./perspectives";

describe("affected", () => {
  it("puts a ref move on the graph and the references", () => {
    expect(affected("refs")).toEqual(["refs", "graph"]);
  });

  it("puts an index change on the panels that show the index", () => {
    expect(affected("index")).toContain("files");
    expect(affected("index")).not.toContain("refs");
  });

  it("leaves the panels alone for a hook change, which none of them show", () => {
    expect(affected("hooks")).toEqual([]);
  });
});

describe("mark", () => {
  it("adds the panels a change touches", () => {
    expect([...mark(new Set(), "refs")].sort()).toEqual(["graph", "refs"]);
  });

  it("keeps what was already stale", () => {
    const before = new Set<PanelId>(["files"]);
    expect(mark(before, "refs").has("files")).toBe(true);
  });

  it("leaves the set it was given alone", () => {
    const before = new Set<PanelId>();
    mark(before, "refs");
    expect(before.size).toBe(0);
  });
});

describe("clear", () => {
  it("drops the panels that reloaded", () => {
    const stale = new Set<PanelId>(["graph", "files"]);
    expect([...clear(stale, ["graph"])]).toEqual(["files"]);
  });

  it("is quiet about a panel that was never stale", () => {
    expect([...clear(new Set<PanelId>(["graph"]), ["diff"])]).toEqual(["graph"]);
  });
});
