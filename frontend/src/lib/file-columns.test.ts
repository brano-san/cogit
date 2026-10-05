import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { groupByDirectory } from "./file-view";
import {
  DEFAULT_COLUMNS,
  DEFAULT_SORT,
  columnItems,
  columnReason,
  directoryOf,
  extensionOf,
  lfsLabel,
  gridColumns,
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

  it("keeps folders in path order in the tree and sorts inside each", () => {
    const sorted = sortRows(files, { key: "name", descending: true }, true);
    expect(paths(sorted)).toEqual(["beta.txt", "docs/alpha.md", "src/zeta.rs", "src/lib/alpha.rs"]);
  });

  // Shift+click ticks a range in the order of the list; the tree has to draw that order.
  it("lists the rows in the order the tree draws them", () => {
    const tree = [entry("b.txt"), entry("gen/a.txt", "untracked"), entry("a.txt"), entry("src/x.rs")];
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
  it("shows the folder a row is in", () => {
    expect(directoryOf("src/lib/a.ts")).toBe("src/lib/");
    expect(directoryOf("README.md")).toBe("");
  });
});

describe("the columns shown", () => {
  it("shows three by default, the name always", () => {
    expect(shownColumns(DEFAULT_COLUMNS, false)).toEqual(["name", "change", "path"]);
    expect(shownColumns({ extension: false, change: false, lfs: false, path: false }, false)).toEqual(["name"]);
  });

  it("leaves the path to the folder rows of the tree", () => {
    expect(shownColumns(DEFAULT_COLUMNS, true)).toEqual(["name", "change"]);
    expect(columnReason("path", true)).toMatch(/folder/);
    expect(columnReason("path", false)).toBeNull();
    expect(columnReason("name", false)).toMatch(/always/);
  });

  it("formats pixel widths for grid columns", () => {
    expect(gridColumns(["name", "change", "path"])).toBe("180px 130px 260px");
    expect(gridColumns(["name", "change"], { name: 200, change: 80 })).toBe("200px 80px");
  });
});

describe("the stored table settings", () => {
  it("falls back to the defaults for anything missing or malformed", () => {
    expect(mergeTable(null)).toEqual({ columns: DEFAULT_COLUMNS, sort: DEFAULT_SORT, widths: expect.any(Object) });
    expect(mergeTable({ columns: { change: false, path: "no" }, sort: { key: "size", descending: true } })).toEqual({
      columns: { ...DEFAULT_COLUMNS, change: false },
      sort: DEFAULT_SORT,
      widths: expect.any(Object),
    });
    expect(mergeTable({ sort: { key: "path", descending: true } }).sort).toEqual({ key: "path", descending: true });
    expect(mergeTable({ widths: { name: 300 } }).widths.name).toBe(300);
  });
});

// Customise View opened on nothing to customise: a switch under a column's name and a
// Size that was always off (#34).
describe("the columns in Customise View", () => {
  it("lists the columns, the name always on and not to be turned off", () => {
    expect(columnItems({ extension: false, change: true, lfs: true, path: true }, false)).toEqual([
      { key: "name", label: "Name", checked: true, reason: "The name is always shown" },
      { key: "extension", label: "Extension", checked: false, reason: null },
      { key: "change", label: "State", checked: true, reason: null },
      { key: "lfs", label: "LFS", checked: true, reason: null },
      { key: "path", label: "Path", checked: true, reason: null },
    ]);
  });

  it("says why the path cannot be shown in the tree, and shows it unticked there", () => {
    const path = columnItems(DEFAULT_COLUMNS, true).find((item) => item.key === "path");
    expect(path).toMatchObject({ checked: false, reason: expect.stringMatching(/folder/) });
  });

  it("turns one column over and leaves the others", () => {
    expect(toggleColumn(DEFAULT_COLUMNS, "change")).toEqual({ ...DEFAULT_COLUMNS, change: false });
    expect(toggleColumn(DEFAULT_COLUMNS, "name")).toEqual(DEFAULT_COLUMNS);
  });
});

describe("the extension and LFS columns", () => {
  it("shows the extension without its dot", () => {
    expect(extensionOf("src/app.Test.ts")).toBe("ts");
    expect(extensionOf(".gitignore")).toBe("");
    expect(extensionOf("Makefile")).toBe("");
  });

  it("sorts by extension, then by name", () => {
    const files = [entry("b.rs"), entry("a.ts"), entry("c.md"), entry("a.rs")];
    expect(paths(sortRows(files, { key: "extension", descending: false }, false))).toEqual([
      "c.md",
      "a.rs",
      "b.rs",
      "a.ts",
    ]);
  });

  it("names the lock, else whether the file can be locked", () => {
    expect(lfsLabel(undefined)).toBe("");
    expect(lfsLabel({ lockable: false, lock: null })).toBe("LFS");
    expect(lfsLabel({ lockable: true, lock: null })).toBe("lockable");
    expect(lfsLabel({ lockable: true, lock: "ann" })).toBe("locked by ann");
    expect(lfsLabel({ lockable: true, lock: "" })).toBe("locked");
  });

  it("sorts by the LFS cell", () => {
    const states = new Map([
      ["a.psd", { lockable: true, lock: null }],
      ["b.psd", { lockable: true, lock: "ann" }],
    ]);
    const sorted = sortRows([entry("b.psd"), entry("a.psd"), entry("c.txt")], { key: "lfs", descending: false }, false, (path) =>
      states.get(path),
    );
    expect(paths(sorted)).toEqual(["c.txt", "a.psd", "b.psd"]);
  });

  it("are hidden by default", () => {
    expect(DEFAULT_COLUMNS.extension).toBe(false);
    expect(DEFAULT_COLUMNS.lfs).toBe(false);
  });
});
