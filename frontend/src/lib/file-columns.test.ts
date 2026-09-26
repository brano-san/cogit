import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { groupByDirectory } from "./file-view";
import {
  DEFAULT_COLUMNS,
  DEFAULT_SORT,
  columnItems,
  columnReason,
  directoryOf,
  fileType,
  gridColumns,
  isBinaryPath,
  mergeTable,
  nextSort,
  shownColumns,
  sortRows,
  toggleColumn,
} from "./file-columns";

function entry(path: string, status: FileEntry["status"] = "modified", mode: FileEntry["mode"] = "plain"): FileEntry {
  return { path, oldPath: null, status, mode, modeChange: null, similarity: null };
}

const paths = (files: readonly FileEntry[]) => files.map((file) => file.path);

describe("the type of a row", () => {
  it("names a submodule a repository, by its mode", () => {
    expect(fileType(entry("import/lib", "modified", "submodule"))).toBe("repository");
  });

  it("knows a binary file by its extension, without reading it", () => {
    expect(fileType(entry("assets/logo.PNG"))).toBe("binary");
    expect(fileType(entry("build/app.exe"))).toBe("binary");
    expect(fileType(entry("src/main.rs"))).toBe("file");
    expect(fileType(entry("Makefile"))).toBe("file");
  });

  it("keeps a symbolic link and an untracked folder apart from files", () => {
    expect(fileType(entry("latest", "modified", "symlink"))).toBe("symlink");
    expect(fileType(entry("generated/", "untracked"))).toBe("directory");
  });

  it("does not take a dot folder for an extension", () => {
    expect(isBinaryPath("cache.png/readme")).toBe(false);
    expect(isBinaryPath(".png")).toBe(false);
  });
});

describe("sorting the rows", () => {
  const files = [
    entry("src/zeta.rs", "added"),
    entry("docs/alpha.md", "deleted"),
    entry("beta.txt", "conflicted"),
    entry("src/lib/alpha.rs", "untracked"),
  ];

  it("goes by name by default, then by path for the same name", () => {
    expect(DEFAULT_SORT).toEqual({ key: "name", descending: false });
    expect(paths(sortRows(files, DEFAULT_SORT, false))).toEqual([
      "docs/alpha.md",
      "src/lib/alpha.rs",
      "beta.txt",
      "src/zeta.rs",
    ]);
  });

  it("sorts by path, and turns round when asked", () => {
    const byPath = paths(sortRows(files, { key: "path", descending: false }, false));
    expect(byPath).toEqual(["beta.txt", "docs/alpha.md", "src/lib/alpha.rs", "src/zeta.rs"]);
    expect(paths(sortRows(files, { key: "path", descending: true }, false))).toEqual([...byPath].reverse());
  });

  it("puts conflicts first when sorted by state", () => {
    expect(paths(sortRows(files, { key: "change", descending: false }, false))).toEqual([
      "beta.txt",
      "src/zeta.rs",
      "docs/alpha.md",
      "src/lib/alpha.rs",
    ]);
  });

  it("groups by type, then by name", () => {
    const mixed = [entry("b.png"), entry("a.txt"), entry("sub", "modified", "submodule"), entry("a.png")];
    expect(paths(sortRows(mixed, { key: "type", descending: false }, false))).toEqual(["a.txt", "sub", "a.png", "b.png"]);
  });

  it("keeps folders in path order in the tree and sorts inside each", () => {
    const sorted = sortRows(files, { key: "name", descending: true }, true);
    expect(paths(sorted)).toEqual(["beta.txt", "docs/alpha.md", "src/zeta.rs", "src/lib/alpha.rs"]);
  });

  // Shift+click ticks a range in the order of the list; the tree has to draw that order.
  it("lists the rows in the order the tree draws them, an untracked folder included", () => {
    const tree = [entry("b.txt"), entry("gen/", "untracked"), entry("a.txt"), entry("src/x.rs")];
    const sorted = sortRows(tree, DEFAULT_SORT, true);
    const drawn = groupByDirectory(sorted, true).flatMap((row) => (row.kind === "file" ? [row.file.path] : []));
    expect(paths(sorted)).toEqual(drawn);
  });

  it("does not touch the list it was given", () => {
    const before = paths(files);
    sortRows(files, { key: "path", descending: true }, false);
    expect(paths(files)).toEqual(before);
  });
});

describe("a click on a column heading", () => {
  it("sorts by that column, and a second click turns the order round", () => {
    expect(nextSort(DEFAULT_SORT, "path")).toEqual({ key: "path", descending: false });
    expect(nextSort({ key: "path", descending: false }, "path")).toEqual({ key: "path", descending: true });
    expect(nextSort({ key: "path", descending: true }, "path")).toEqual({ key: "path", descending: false });
  });
});

describe("the path column", () => {
  it("shows the folder a row is in, not an untracked folder itself", () => {
    expect(directoryOf("src/lib/a.ts")).toBe("src/lib/");
    expect(directoryOf("README.md")).toBe("");
    expect(directoryOf("generated/")).toBe("");
    expect(directoryOf("src/generated/")).toBe("src/");
  });
});

describe("the columns shown", () => {
  it("shows all four by default, the name always", () => {
    expect(shownColumns(DEFAULT_COLUMNS, false)).toEqual(["name", "type", "change", "path"]);
    expect(shownColumns({ type: false, change: false, path: false }, false)).toEqual(["name"]);
  });

  it("leaves the path to the folder rows of the tree", () => {
    expect(shownColumns(DEFAULT_COLUMNS, true)).toEqual(["name", "type", "change"]);
    expect(columnReason("path", true)).toMatch(/folder/);
    expect(columnReason("path", false)).toBeNull();
    expect(columnReason("name", false)).toMatch(/always/);
  });

  it("starts every path at one vertical: fixed columns between the name and the path", () => {
    expect(gridColumns(["name", "type", "change", "path"])).toBe(
      "minmax(0, 2fr) var(--file-type-width) var(--file-change-width) minmax(0, 3fr)",
    );
    expect(gridColumns(["name", "change"])).toBe("minmax(0, 1fr) var(--file-change-width)");
  });
});

describe("the stored table settings", () => {
  it("falls back to the defaults for anything missing or malformed", () => {
    expect(mergeTable(null)).toEqual({ columns: DEFAULT_COLUMNS, sort: DEFAULT_SORT });
    expect(mergeTable({ columns: { type: false, path: "no" }, sort: { key: "size", descending: true } })).toEqual({
      columns: { ...DEFAULT_COLUMNS, type: false },
      sort: DEFAULT_SORT,
    });
    expect(mergeTable({ sort: { key: "path", descending: true } }).sort).toEqual({ key: "path", descending: true });
  });
});

// Customise View opened on nothing to customise: a switch under a column's name and a
// Size that was always off (#34).
describe("the columns in Customise View", () => {
  it("lists the four columns, the name always on and not to be turned off", () => {
    expect(columnItems({ type: false, change: true, path: true }, false)).toEqual([
      { key: "name", label: "Name", checked: true, reason: "The name is always shown" },
      { key: "type", label: "Type", checked: false, reason: null },
      { key: "change", label: "State", checked: true, reason: null },
      { key: "path", label: "Path", checked: true, reason: null },
    ]);
  });

  it("says why the path cannot be shown in the tree, and shows it unticked there", () => {
    const path = columnItems(DEFAULT_COLUMNS, true).find((item) => item.key === "path");
    expect(path).toMatchObject({ checked: false, reason: expect.stringMatching(/folder/) });
  });

  it("turns one column over and leaves the others", () => {
    expect(toggleColumn(DEFAULT_COLUMNS, "type")).toEqual({ ...DEFAULT_COLUMNS, type: false });
    expect(toggleColumn(DEFAULT_COLUMNS, "name")).toEqual(DEFAULT_COLUMNS);
  });
});
