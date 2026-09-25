import { describe, expect, it } from "vitest";
import { compile, haystack, matches } from "./file-search";
import type { FileEntry } from "./ipc/bindings";

function file(over: Partial<FileEntry> = {}): FileEntry {
  return {
    path: "src/lib/graph.ts",
    oldPath: null,
    status: "modified",
    modeChange: null,
    similarity: null,
    ...over,
  } as FileEntry;
}

describe("compile", () => {
  it("matches nothing in particular when the text is empty", () => {
    expect(compile("   ", false).test).toBeNull();
    expect(compile("", true).broken).toBe(false);
  });

  it("matches a substring, case insensitively", () => {
    const pattern = compile("GRAPH", false);
    expect(pattern.test?.("src/graph.ts")).toBe(true);
    expect(pattern.test?.("src/lanes.ts")).toBe(false);
  });

  it("takes a bracket literally outside regex mode", () => {
    expect(compile("(", false).broken).toBe(false);
    expect(compile("(", false).test?.("a(b")).toBe(true);
  });

  it("reads the text as an expression in regex mode", () => {
    const pattern = compile("^src/.*\\.ts$", true);
    expect(pattern.test?.("src/graph.ts")).toBe(true);
    expect(pattern.test?.("doc/graph.ts")).toBe(false);
  });

  it("reports a half-typed expression instead of throwing", () => {
    const pattern = compile("src/(", true);
    expect(pattern.broken).toBe(true);
    expect(pattern.test).toBeNull();
  });

  it("a broken expression hides nothing", () => {
    expect(matches(file(), compile("src/(", true))).toBe(true);
  });
});

describe("haystack", () => {
  it("carries the name, the path and the state", () => {
    const parts = haystack(file());
    expect(parts).toContain("graph.ts");
    expect(parts).toContain("src/lib/graph.ts");
    expect(parts.map((part) => part.toLowerCase())).toContain("modified");
  });

  it("carries the old path of a rename too", () => {
    expect(haystack(file({ oldPath: "src/old-name.ts" }))).toContain("src/old-name.ts");
  });
});

describe("matches", () => {
  it("finds a file by its bare name", () => {
    expect(matches(file(), compile("graph.ts", false))).toBe(true);
  });

  it("finds a file by a folder in its path", () => {
    expect(matches(file(), compile("lib/", false))).toBe(true);
  });

  it("finds a file by its state, with no switch to set first", () => {
    expect(matches(file({ status: "untracked" }), compile("untracked", false))).toBe(true);
    expect(matches(file({ status: "modified" }), compile("untracked", false))).toBe(false);
  });

  it("keeps everything when nothing is typed", () => {
    expect(matches(file(), compile("", false))).toBe(true);
  });

  it("anchors an expression to the name, the path or the state, not to their join", () => {
    const svelte = file({ path: "src/App.svelte" });
    expect(matches(svelte, compile("\\.svelte$", true))).toBe(true);
    expect(matches(svelte, compile("^src/", true))).toBe(true);
    expect(matches(svelte, compile("^modified$", true))).toBe(true);
    expect(matches(svelte, compile("^App\\.svelte src", true))).toBe(false);
  });

  it("reads a mask with * or ? as a glob over the name or the path", () => {
    const rust = file({ path: "src/deep/main.rs" });
    expect(matches(rust, compile("*.rs", false))).toBe(true);
    expect(matches(rust, compile("*.RS", false))).toBe(true);
    expect(matches(rust, compile("src/*.rs", false))).toBe(true);
    expect(matches(rust, compile("main.r?", false))).toBe(true);
    expect(matches(rust, compile("*.ts", false))).toBe(false);
    expect(matches(file({ path: "src/main.rs.bak" }), compile("*.rs", false))).toBe(false);
  });

  it("finds no text across the seam between two parts", () => {
    expect(matches(file({ path: "src/App.svelte" }), compile("svelte src", false))).toBe(false);
  });
});
