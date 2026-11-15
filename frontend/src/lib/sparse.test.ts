import { describe, expect, it } from "vitest";
import {
  applyLabel,
  coneSet,
  folderCheck,
  folderRows,
  parsePatterns,
  planOf,
  previewLine,
  sparseTooltip,
  toggleFolder,
} from "./sparse";
import type { SparseFolder, SparseState } from "./ipc/bindings";

/** a/{b/{c,d},e}, f */
const TREE: Record<string, string[]> = {
  "": ["a", "f"],
  a: ["a/b", "a/e"],
  "a/b": ["a/b/c", "a/b/d"],
};
const children = (dir: string) => TREE[dir] ?? [];

describe("sparse checkout tree", () => {
  it("keeps only the topmost ticked folders, without slashes", () => {
    expect(coneSet(["a/b", "/a/", "f", "a/b/c", "f/"])).toEqual(["a", "f"]);
    expect(coneSet(["ab", "a"])).toEqual(["a", "ab"]);
  });

  it("shows a folder on under a ticked one and mixed above one", () => {
    const set = ["a/b"];
    expect(folderCheck(set, "a/b")).toBe("on");
    expect(folderCheck(set, "a/b/c")).toBe("on");
    expect(folderCheck(set, "a")).toBe("mixed");
    expect(folderCheck(set, "a/e")).toBe("off");
    expect(folderCheck(["ab"], "a")).toBe("off");
  });

  it("ticking a folder takes its subtree in place of what was ticked inside it", () => {
    expect(toggleFolder(["a/b/c", "f"], "a", children)).toEqual(["a", "f"]);
    expect(toggleFolder([], "a/b", children)).toEqual(["a/b"]);
  });

  it("unticking a ticked folder takes it out", () => {
    expect(toggleFolder(["a", "f"], "a", children)).toEqual(["f"]);
  });

  it("unticking a folder inside a ticked one keeps its neighbours on each level", () => {
    expect(toggleFolder(["a"], "a/b/c", children)).toEqual(["a/b/d", "a/e"]);
    expect(toggleFolder(["a"], "a/e", children)).toEqual(["a/b"]);
  });

  it("clicking a mixed folder ticks the whole of it", () => {
    expect(toggleFolder(["a/b/c"], "a", children)).toEqual(["a"]);
  });
});

describe("folder rows", () => {
  const folder = (path: string, folders = false): SparseFolder => ({ path, folders });
  const read = new Map<string, SparseFolder[]>([
    ["", [folder("a", true), folder("f")]],
    ["a", [folder("a/b", true), folder("a/e")]],
  ]);

  it("lists the open folders depth first", () => {
    expect(folderRows(read, new Set(["a"])).map((row) => [row.name, row.depth, row.open])).toEqual([
      ["a", 0, true],
      ["b", 1, false],
      ["e", 1, false],
      ["f", 0, false],
    ]);
  });

  it("shows nothing under an open folder not read yet", () => {
    expect(folderRows(read, new Set(["a", "a/b"])).map((row) => row.path)).toEqual(["a", "a/b", "a/e", "f"]);
  });
});

describe("sparse plan", () => {
  const off: SparseState = { enabled: false, cone: true, set: [] };
  const on: SparseState = { enabled: true, cone: true, set: ["a"] };
  const patterns: SparseState = { enabled: true, cone: false, set: ["/*", "!/*/"] };

  it("enables cone mode with the ticked folders", () => {
    expect(planOf(off, true, ["a/b", "a"])).toEqual({ kind: "set", cone: true, set: ["a"] });
    expect(applyLabel(off, planOf(off, true, []))).toBe("Enable");
  });

  it("has nothing to do when the set is the same", () => {
    expect(planOf(on, true, ["a"])).toBeNull();
    expect(planOf(off, false, [])).toBeNull();
  });

  it("disables when the box is cleared", () => {
    expect(planOf(on, false, ["a"])).toEqual({ kind: "disable" });
    expect(applyLabel(on, { kind: "disable" })).toBe("Disable");
  });

  it("keeps patterns as typed in non-cone mode", () => {
    expect(planOf(patterns, true, ["/*", "!/*/", "/docs/"])).toEqual({
      kind: "set",
      cone: false,
      set: ["/*", "!/*/", "/docs/"],
    });
  });

  it("reads one pattern per line, past blanks and comments", () => {
    expect(parsePatterns("/*\r\n\n# docs\n!/*/  \n")).toEqual(["/*", "!/*/"]);
  });

  it("says what comes and goes", () => {
    expect(previewLine({ tracked: 10, present: 4, removed: 6, appearing: 0, blocked: [] })).toBe(
      "4 files of 10 in the working tree · 6 files removed",
    );
    expect(previewLine({ tracked: 10, present: 10, removed: 0, appearing: 1, blocked: [] })).toBe(
      "10 files of 10 in the working tree · 1 file brought back",
    );
    expect(previewLine({ tracked: 2, present: 2, removed: 0, appearing: 0, blocked: [] })).toBe(
      "2 files of 2 in the working tree · no file comes or goes",
    );
  });

  it("names the set's size in the row's tooltip", () => {
    expect(sparseTooltip(1)).toBe("Sparse checkout is on: 1 entry in its set");
    expect(sparseTooltip(3)).toBe("Sparse checkout is on: 3 entries in its set");
    expect(sparseTooltip(0)).toBe("Sparse checkout is on: only the top-level files");
  });
});
