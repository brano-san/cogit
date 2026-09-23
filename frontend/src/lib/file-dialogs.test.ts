import { describe, expect, it } from "vitest";
import { editedSides, editorSide, forDisk, listedMessage, removeRows } from "./file-dialogs";

describe("removeRows", () => {
  it("splits each path into name and directory, in path order", () => {
    expect(removeRows(["src/lib/a.ts", "README.md", "src/lib/a.ts"])).toEqual([
      { path: "README.md", name: "README.md", directory: "" },
      { path: "src/lib/a.ts", name: "a.ts", directory: "src/lib" },
    ]);
  });
});

describe("listedMessage", () => {
  it("leaves a question about one file alone", () => {
    expect(listedMessage("Delete a.txt?", ["a.txt"])).toBe("Delete a.txt?");
  });

  it("names several files under the question and counts the ones that do not fit", () => {
    const paths = Array.from({ length: 14 }, (_, at) => `f${at}.txt`);
    const lines = listedMessage("Delete 14 files?", paths).split("\n");
    expect(lines.slice(0, 3)).toEqual(["Delete 14 files?", "", "f0.txt"]);
    expect(lines).toHaveLength(2 + 12 + 1);
    expect(lines.at(-1)).toBe("and 2 more");
  });
});

describe("the Index Editor's line endings", () => {
  it("edits with LF and writes CRLF back to a file that had it", () => {
    const side = editorSide("one\r\ntwo\r\n");
    expect(side).toEqual({ text: "one\ntwo\n", crlf: true, present: true });
    expect(forDisk(side, "one\nthree\n")).toBe("one\r\nthree\r\n");
    expect(forDisk(editorSide("a\n"), "b\n")).toBe("b\n");
  });

  it("knows an absent side from an empty one", () => {
    expect(editorSide(null).present).toBe(false);
    expect(editorSide("").present).toBe(true);
  });

  it("writes only the sides that changed", () => {
    const sides = { index: editorSide("a\n"), worktree: editorSide("b\r\n") };
    expect(editedSides(sides, { index: "a\n", worktree: "b\n" })).toEqual({ index: null, worktree: null });
    expect(editedSides(sides, { index: "c\n", worktree: "b\n" })).toEqual({ index: "c\n", worktree: null });
    expect(editedSides(sides, { index: "a\n", worktree: "d\n" })).toEqual({ index: null, worktree: "d\r\n" });
  });
});
