import { describe, expect, it } from "vitest";
import {
  changesFirst,
  DEFAULT_VIEW,
  backendView,
  foldersOf,
  groupByDirectory,
  hiddenFiles,
  mergeView,
  paneLayout,
  readsAgain,
  shownSections,
  visibleFiles,
  type FileView,
} from "./file-view";
import type { FileEntry } from "./ipc/bindings";

const file = (path: string, status: FileEntry["status"], oldPath: string | null = null): FileEntry =>
  ({ path, status, oldPath, modeChange: null, similarity: null }) as FileEntry;

const view = (over: Partial<FileView> = {}): FileView => ({ ...DEFAULT_VIEW, ...over });

// Show Directories, Missing and the rest read the whole status again, though R-43 says only
// the four backend switches go to the backend.
describe("readsAgain", () => {
  it("is false for a switch the list applies itself", () => {
    expect(readsAgain(view(), view({ directories: true }))).toBe(false);
    expect(readsAgain(view(), view({ renameSources: true, untracked: false }))).toBe(false);
  });

  it("is true for one of the four the backend reads", () => {
    expect(readsAgain(view(), view({ unchanged: true }))).toBe(true);
    expect(readsAgain(view(), view({ skipped: true }))).toBe(true);
  });
});

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

  // The toolbar has no switch of its own for them; without this a file flagged from the
  // menu vanished, and the flag could not be cleared.
  it("asks for assume-unchanged files along with skipped ones", () => {
    expect(backendView(view({ skipped: true }))).toMatchObject({ assumeUnchanged: true, skipped: true });
  });
});

describe("visibleFiles with the Skipped switch", () => {
  const flagged = [file("skip.rs", "skipped"), file("assume.rs", "assumeUnchanged"), file("a.rs", "modified")];

  it("shows skip-worktree and assume-unchanged files while it is on", () => {
    expect(visibleFiles(flagged, view({ skipped: true })).map((f) => f.path)).toEqual([
      "skip.rs",
      "assume.rs",
      "a.rs",
    ]);
  });

  it("hides both while it is off", () => {
    expect(visibleFiles(flagged, view({ skipped: false })).map((f) => f.path)).toEqual(["a.rs"]);
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

  // A copy's source is still there: it was listed as deleted, and twice when it changed too.
  it("does not list the source of a copy as removed", () => {
    const files = [file("b.rs", "copied", "a.rs"), file("a.rs", "modified")];
    const shown = visibleFiles(files, view({ renameSources: true }));
    expect(shown.map((f) => [f.path, f.status])).toEqual([
      ["b.rs", "copied"],
      ["a.rs", "modified"],
    ]);
  });

  it("does not invent a source for a file that was not renamed", () => {
    const shown = visibleFiles([file("a.rs", "modified")], view({ renameSources: true }));
    expect(shown).toHaveLength(1);
  });
});

// R-871: all the files of the revision less the ones shown, whatever hid them.
describe("hiddenFiles", () => {
  it("is every file of a commit while the list shows none", () => {
    expect(hiddenFiles(120, [], [], false)).toBe(120);
  });

  it("is the tree less the files a commit's list shows, not counting its deletions", () => {
    const shown = [file("a.rs", "modified"), file("b.rs", "added"), file("gone.rs", "deleted")];
    expect(hiddenFiles(120, shown, shown, false)).toBe(118);
  });

  it("adds the untracked files to the index of the working tree and leaves out ignored rows", () => {
    const came = [file("a.rs", "modified"), file("new.rs", "untracked"), file("out/app.js", "ignored")];
    expect(hiddenFiles(10, came, came.slice(0, 1), true)).toBe(10);
    expect(hiddenFiles(10, came, came, true)).toBe(9);
  });

  it("counts a path shown in two sections once and never goes below zero", () => {
    const twice = [file("a.rs", "modified"), file("a.rs", "modified")];
    expect(hiddenFiles(1, twice, twice, true)).toBe(0);
    expect(hiddenFiles(0, twice, twice, false)).toBe(0);
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
    expect(rows[0]).toEqual({ kind: "dir", path: "", count: 1, open: true });
  });

  it("counts what is under each directory", () => {
    const files = [file("src/a.rs", "modified"), file("src/b.rs", "modified")];
    const rows = groupByDirectory(files, true);
    expect(rows[0]).toEqual({ kind: "dir", path: "src/", count: 2, open: true });
  });

  it("keeps a folded folder's heading and count, and hides its files", () => {
    const files = [file("src/a.rs", "modified"), file("src/b.rs", "modified"), file("top.rs", "modified")];
    const rows = groupByDirectory(files, true, new Set(["src/"]));
    expect(rows).toEqual([
      { kind: "dir", path: "src/", count: 2, open: false },
      { kind: "dir", path: "", count: 1, open: true },
      { kind: "file", file: files[2] },
    ]);
    expect(foldersOf(rows)).toEqual(["src/", ""]);
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

describe("changesFirst", () => {
  it("puts unchanged files under the changes, each kept in its order", () => {
    const rows = [
      { path: "a", status: "unchanged" },
      { path: "b", status: "modified" },
      { path: "c", status: "unchanged" },
      { path: "d", status: "untracked" },
    ] as const;
    expect(changesFirst(rows).map((file) => file.path)).toEqual(["b", "d", "a", "c"]);
  });
});
