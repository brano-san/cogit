import { describe, expect, it } from "vitest";
import type { IgnoreHit, IgnorePreview } from "$lib/ipc";
import { hitSource, lineNotes, parentDirs } from "./ignore-editor";

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
