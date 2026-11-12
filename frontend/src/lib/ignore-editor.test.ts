import { describe, expect, it } from "vitest";
import type { ContextItem, IgnoreHit, IgnorePreview } from "$lib/ipc";
import {
  editLine,
  groupHits,
  hitSource,
  ignoreMenu,
  lineNotes,
  negation,
  parentDirs,
  ruleAt,
  ruleText,
  type HitGroup,
} from "./ignore-editor";

const hit = (line: number, extra: Partial<IgnoreHit> = {}): IgnoreHit => ({
  path: "a.log",
  tracked: false,
  scope: "folder",
  dir: "",
  line,
  pattern: "*.log",
  underFolder: null,
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

describe("ruleText", () => {
  it("reads a line the way git does", () => {
    expect(ruleText("build/")).toBe("build/");
    expect(ruleText("*.log   ")).toBe("*.log");
    expect(ruleText(String.raw`a\ `)).toBe(String.raw`a\ `);
    expect(ruleText(String.raw`\#hash`)).toBe("#hash");
    expect(ruleText("!/keep")).toBe("!/keep");
    expect(ruleText("# note")).toBeNull();
    expect(ruleText("   ")).toBeNull();
  });
});

describe("ruleAt", () => {
  it("holds only while the line still says the pattern", () => {
    expect(ruleAt("a\n*.log  \nb\n", 2, "*.log")).toBe(true);
    expect(ruleAt("*.log\na\nb\n", 2, "*.log")).toBe(false);
    expect(ruleAt("a\n# *.log\n", 2, "*.log")).toBe(false);
    expect(ruleAt("a\n", 7, "*.log")).toBe(false);
  });
});

describe("ignoreMenu", () => {
  const group = (extra: Partial<HitGroup> = {}): HitGroup => ({
    key: "k",
    source: ".gitignore:1",
    scope: "folder",
    dir: "",
    line: 1,
    pattern: "build/",
    hits: [],
    ...extra,
  });
  const row = (items: ContextItem[], id: string) => items.find((each) => each.id === id);
  const under = hit(1, { path: "build/a.o", pattern: "build/", underFolder: "build" });

  it("says a file under an ignored folder cannot be kept", () => {
    const keep = row(ignoreMenu(group(), under, "build/\n", true), "ignore-keep")!;
    expect(keep.enabled).toBe(false);
    expect(keep.label).toBe("Don't Ignore a.o (its folder is ignored)");
  });

  it("keeps a file the rule matches itself", () => {
    const keep = row(ignoreMenu(group({ pattern: "*.log" }), hit(1), "*.log\n", true), "ignore-keep")!;
    expect(keep.enabled).toBe(true);
  });

  it("turns the rule rows off while the list is out of date or the line moved", () => {
    const rules = ["ignore-go", "ignore-comment", "ignore-remove"];
    const stale = ignoreMenu(group(), null, "build/\n", false);
    expect(rules.map((id) => row(stale, id)!.enabled)).toEqual([false, false, false]);
    const moved = ignoreMenu(group(), null, "x\nbuild/\n", true);
    expect(rules.map((id) => row(moved, id)!.enabled)).toEqual([false, false, false]);
    const fine = ignoreMenu(group(), null, "build/\n", true);
    expect(rules.map((id) => row(fine, id)!.enabled)).toEqual([true, true, true]);
  });

  it("starts and ends with a row", () => {
    const items = ignoreMenu(group(), null, null, true);
    expect(items[0]!.separator).toBe(false);
    expect(items[items.length - 1]!.separator).toBe(false);
  });
});
