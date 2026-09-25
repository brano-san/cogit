import { describe, expect, it } from "vitest";
import {
  DEFAULT_VIEW,
  backendView,
  groupByDirectory,
  mergeView,
  paneLayout,
  shownSections,
  visibleFiles,
  type FileView,
} from "./file-view";
import type { FileEntry } from "./ipc/bindings";

const file = (path: string, status: FileEntry["status"], oldPath: string | null = null): FileEntry =>
  ({ path, status, oldPath, modeChange: null, similarity: null }) as FileEntry;

const view = (over: Partial<FileView> = {}): FileView => ({ ...DEFAULT_VIEW, ...over });

describe("backendView", () => {
  it("asks for nothing extra by default", () => {
    expect(backendView(DEFAULT_VIEW)).toEqual({
      unchanged: false,
      ignored: false,
      assumeUnchanged: false,
      skipped: false,
    });
  });

  it("passes on only the four that cost extra reading", () => {
    const asked = backendView(view({ unchanged: true, ignored: true, directories: true }));
    expect(asked).toEqual({
      unchanged: true,
      ignored: true,
      assumeUnchanged: false,
      skipped: false,
    });
  });
});

describe("visibleFiles", () => {
  const files = [
    file("a.rs", "modified"),
    file("new.rs", "untracked"),
    file("quiet.rs", "unchanged"),
    file("dist/app.js", "ignored"),
    file("moved.rs", "renamed", "old.rs"),
  ];

  it("hides untracked files when the switch is off", () => {
    const shown = visibleFiles(files, view({ untracked: false })).map((f) => f.path);
    expect(shown).not.toContain("new.rs");
    expect(shown).toContain("a.rs");
  });

  it("keeps untracked files when the switch is on", () => {
    expect(visibleFiles(files, view()).map((f) => f.path)).toContain("new.rs");
  });

  it("hides unchanged and ignored rows the backend sent but the switch turned off", () => {
    const shown = visibleFiles(files, view()).map((f) => f.path);
    expect(shown).not.toContain("quiet.rs");
    expect(shown).not.toContain("dist/app.js");
  });

  it("adds the vanished source of a rename when asked", () => {
    const shown = visibleFiles(files, view({ renameSources: true }));
    const source = shown.find((f) => f.path === "old.rs");
    expect(source?.status).toBe("deleted");
  });

  it("leaves the rename itself in place beside its source", () => {
    const shown = visibleFiles(files, view({ renameSources: true })).map((f) => f.path);
    expect(shown).toContain("moved.rs");
    expect(shown).toContain("old.rs");
  });

  it("does not invent a source for a file that was not renamed", () => {
    const shown = visibleFiles([file("a.rs", "modified")], view({ renameSources: true }));
    expect(shown).toHaveLength(1);
  });
});

describe("groupByDirectory", () => {
  it("returns the files unchanged when the switch is off", () => {
    const files = [file("src/a.rs", "modified"), file("b.rs", "modified")];
    expect(groupByDirectory(files, false)).toEqual([
      { kind: "file", file: files[0] },
      { kind: "file", file: files[1] },
    ]);
  });

  it("puts a heading before each directory", () => {
    const files = [file("src/a.rs", "modified"), file("src/b.rs", "modified"), file("c.rs", "modified")];
    const rows = groupByDirectory(files, true);
    expect(rows.map((row) => (row.kind === "dir" ? row.path : "·"))).toEqual([
      "src/",
      "·",
      "·",
      "",
      "·",
    ]);
  });

  it("names the repository root by an empty path, not by a slash", () => {
    const rows = groupByDirectory([file("c.rs", "modified")], true);
    expect(rows[0]).toEqual({ kind: "dir", path: "", count: 1 });
  });

  it("counts what is under each directory", () => {
    const files = [file("src/a.rs", "modified"), file("src/b.rs", "modified")];
    const rows = groupByDirectory(files, true);
    expect(rows[0]).toEqual({ kind: "dir", path: "src/", count: 2 });
  });
});

describe("mergeView", () => {
  it("falls back to the defaults for anything stored wrong", () => {
    expect(mergeView({ untracked: "yes", ignored: true })).toEqual({
      ...DEFAULT_VIEW,
      ignored: true,
    });
  });

  it("survives a missing store", () => {
    expect(mergeView(null)).toEqual(DEFAULT_VIEW);
  });
});

describe("shownSections", () => {
  const unstaged = { title: "Unstaged", files: [file("a.txt", "modified")] };
  const staged = { title: "Staged", files: [] as FileEntry[], hideWhenEmpty: true };

  it("leaves out an empty section that asks to be hidden", () => {
    expect(shownSections([unstaged, staged]).map((s) => s.title)).toEqual(["Unstaged"]);
  });

  it("keeps it once it has a file", () => {
    const filled = { ...staged, files: [file("b.txt", "added")] };
    expect(shownSections([unstaged, filled]).map((s) => s.title)).toEqual(["Unstaged", "Staged"]);
  });

  it("keeps an empty section that did not ask", () => {
    const empty = { title: "Unstaged", files: [] as FileEntry[] };
    expect(shownSections([empty, staged]).map((s) => s.title)).toEqual(["Unstaged"]);
  });
});

describe("paneLayout", () => {
  it("keeps the headings, and Stage all with them, when an empty Staged is hidden", () => {
    expect(paneLayout(2, 1, true)).toEqual({ apart: true, titled: true });
    expect(paneLayout(2, 1, false)).toEqual({ apart: false, titled: true });
  });

  it("splits two visible sections into two panes when asked", () => {
    expect(paneLayout(2, 2, true)).toEqual({ apart: true, titled: true });
  });

  it("gives a single-section list, such as a commit's files, no heading", () => {
    expect(paneLayout(1, 1, true)).toEqual({ apart: false, titled: false });
  });
});
