import { describe, expect, it } from "vitest";
import type { IgnoreHit, IgnorePreview } from "$lib/ipc";
import { editLine, groupHits, hitSource, lineNotes, negation, parentDirs } from "./ignore-editor";

const hit = (line: number, extra: Partial<IgnoreHit> = {}): IgnoreHit => ({
  path: "a.log",
  tracked: false,
  scope: "folder",
  dir: "",
  line,
  pattern: "*.log",
  ...extra,
});

const preview = (hits: IgnoreHit[], total = hits.length): IgnorePreview => ({ hits, total });

describe("lineNotes", () => {
  it("marks repeated patterns and the ones nothing matches", () => {
    const notes = lineNotes("# head\n*.log\nbuild/\n*.log\n!keep.log\n\n", { scope: "folder", dir: "" }, preview([hit(2)]));
    expect([...notes]).toEqual([
      [3, "unused"],
      [4, "duplicate"],
    ]);
  });

  it("says nothing of unused lines while the preview is cut short", () => {
    const notes = lineNotes("*.log\nbuild/\n", { scope: "folder", dir: "" }, preview([hit(1)], 900));
    expect([...notes]).toEqual([]);
  });

  it("counts only the hits of its own file", () => {
    const notes = lineNotes("*.log\n", { scope: "exclude", dir: "" }, preview([hit(1)]));
    expect([...notes]).toEqual([[1, "unused"]]);
  });
});

describe("hitSource", () => {
  it("names the file and the line", () => {
    expect(hitSource(hit(3))).toBe(".gitignore:3");
    expect(hitSource(hit(2, { dir: "sub/dir" }))).toBe("sub/dir/.gitignore:2");
    expect(hitSource(hit(1, { scope: "exclude" }))).toBe("info/exclude:1");
    expect(hitSource(hit(4, { scope: "global" }))).toBe("global:4");
  });
});

describe("parentDirs", () => {
  it("walks from the file's folder up to the root", () => {
    expect(parentDirs("a/b/c.txt")).toEqual(["a/b", "a", ""]);
    expect(parentDirs("top.txt")).toEqual([""]);
  });
});

describe("groupHits", () => {
  it("puts the files under the rule that decides them, in file and line order", () => {
    const groups = groupHits([
      hit(3, { path: "b.tmp", pattern: "*.tmp" }),
      hit(1, { path: "a.log" }),
      hit(3, { path: "c.tmp", pattern: "*.tmp" }),
      hit(2, { path: "x", scope: "global", pattern: "x" }),
    ]);
    expect(groups.map((group) => [group.source, group.hits.map((each) => each.path)])).toEqual([
      [".gitignore:1", ["a.log"]],
      [".gitignore:3", ["b.tmp", "c.tmp"]],
      ["global:2", ["x"]],
    ]);
  });
});

describe("negation", () => {
  it("anchors the path to the folder of the file it goes into", () => {
    expect(negation("build/a.o", { scope: "folder", dir: "" })).toBe("!/build/a.o");
    expect(negation("sub/out/a.o", { scope: "folder", dir: "sub" })).toBe("!/out/a.o");
    expect(negation("dist/", { scope: "exclude", dir: "" })).toBe("!/dist/");
  });

  it("escapes what a pattern would read as a wildcard", () => {
    expect(negation("a[1]*.txt", { scope: "folder", dir: "" })).toBe(String.raw`!/a\[1]\*.txt`);
  });

  it("refuses a path outside the folder", () => {
    expect(negation("other/a.o", { scope: "folder", dir: "sub" })).toBeNull();
  });
});

describe("editLine", () => {
  it("comments out, removes and appends", () => {
    expect(editLine("a\n*.log\nb\n", 2, "comment")).toBe("a\n# *.log\nb\n");
    expect(editLine("a\n*.log\nb\n", 2, "remove")).toBe("a\nb\n");
    expect(editLine("a", 0, { append: "!/x" })).toBe("a\n!/x\n");
    expect(editLine("", 0, { append: "!/x" })).toBe("!/x\n");
  });
});
