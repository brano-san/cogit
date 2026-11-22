import { describe, expect, it } from "vitest";
import { workingFileDoubleClick } from "./file-edit";
import { compile } from "./file-search";
import { displayDate, formatDatePattern } from "./format";
import { graphTime } from "./graph-columns";
import { merge } from "./settings";
import { solverAutoAdvance } from "./solver-flow";

describe("working file double-click", () => {
  const file = { status: "modified", mode: "plain" } as const;

  it("defaults to opening the correct side", () => {
    expect(workingFileDoubleClick(file, "index", "open", true)).toEqual({
      kind: "compare", spec: { kind: "indexVsHead" },
    });
  });

  it("stages and unstages only changed rows in the staging mode", () => {
    expect(workingFileDoubleClick(file, "worktree", "stage", true)).toEqual({ kind: "stage" });
    expect(workingFileDoubleClick(file, "index", "stage", true)).toEqual({ kind: "unstage" });
    expect(workingFileDoubleClick({ ...file, status: "unchanged" }, "worktree", "stage", false)).toEqual({ kind: "editor" });
    expect(workingFileDoubleClick({ ...file, status: "conflicted" }, "worktree", "stage", true)).toEqual({ kind: "solver" });
    expect(workingFileDoubleClick({ ...file, mode: "submodule" }, "worktree", "stage", true)).toEqual({ kind: "module" });
  });
});

describe("name filter case", () => {
  it("matches plain text using the selected mode", () => {
    expect(compile("ReadMe", false, "ignore").test?.("README.md")).toBe(true);
    expect(compile("readme", false, "match").test?.("README.md")).toBe(false);
    expect(compile("readme", false, "smart").test?.("README.md")).toBe(true);
    expect(compile("ReadMe", false, "smart").test?.("README.md")).toBe(false);
  });

  it("applies smart case to globs and regular expressions, including Unicode", () => {
    expect(compile("*.rs", false, "smart").test?.("lib.RS")).toBe(true);
    expect(compile("*.Rs", false, "smart").test?.("lib.RS")).toBe(false);
    expect(compile("^é", true, "smart").test?.("Édit")).toBe(true);
    expect(compile("^É", true, "smart").test?.("édit")).toBe(false);
  });
});

describe("commit date formats", () => {
  const timestamp = Date.UTC(2026, 0, 2, 23, 34, 56) / 1000;
  const now = timestamp + 9 * 86_400;
  const pattern = "YYYY/MM/DD 'at' HH:mm:ss Z";

  it("preserves the existing presets and formats a custom date in the commit's offset", () => {
    const custom = "2026/01/03 at 01:04:56 +01:30";
    expect(displayDate(timestamp, 90, now, "smart")).toBe("03-01-26");
    expect(graphTime(timestamp, 90, now, "dateTime")).toBe("03-01-2026 01:04");
    expect(displayDate(timestamp, 90, now, "custom", pattern)).toBe(custom);
    expect(graphTime(timestamp, 90, now, "custom", pattern)).toBe(custom);
  });

  it("rejects unknown tokens, plain literals and unfinished quotes", () => {
    expect(formatDatePattern(timestamp, 0, "YYYY-QQ")).toBeNull();
    expect(formatDatePattern(timestamp, 0, "'date only'")).toBeNull();
    expect(formatDatePattern(timestamp, 0, "YYYY 'date")).toBeNull();
    expect(formatDatePattern(timestamp, -330, "HH:mm Z")).toBe("18:04 -05:30");
  });
});

it("keeps old UI defaults and rejects malformed stored UI options", () => {
  expect(merge({ settingsVersion: 3 })).toMatchObject({
    messageGuides: false, messageSubjectColumn: 50, messageBodyColumn: 72, commitComments: "keep",
    fileDoubleClick: "open", filterCase: "ignore", datePattern: "YYYY-MM-DD HH:mm",
  });
  expect(merge({ messageSubjectColumn: 0, messageBodyColumn: 5.2, datePattern: "QQ" })).toMatchObject({
    messageSubjectColumn: 50, messageBodyColumn: 72, datePattern: "YYYY-MM-DD HH:mm",
  });
});

it("advances a Solver toolbar take past the just-resolved conflict", () => {
  expect(solverAutoAdvance([1, 7], 3)).toBe(7);
  expect(solverAutoAdvance([1], 7)).toBe(1);
  expect(solverAutoAdvance([3], 3)).toBeNull();
  expect(solverAutoAdvance([], 7)).toBeNull();
});
