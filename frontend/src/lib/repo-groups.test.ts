import { describe, expect, it } from "vitest";
import {
  UNGROUPED,
  addGroup,
  assign,
  backendRows,
  groupRows,
  mergeGroups,
  removalQuestion,
  removeGroup,
  renameGroup,
  rowKey,
  showsFilter,
  type RepoGroups,
} from "./repo-groups";
import { NO_FILTER_FOLDS, shownFolds } from "./tree";

const empty: RepoGroups = { order: [], names: {}, of: {}, under: {} };

const withTwo: RepoGroups = {
  order: ["g1", "g2"],
  names: { g1: "Work", g2: "Toys" },
  of: { "/w/alpha": "g1", "/w/beta": "g1", "/w/gamma": "g2" },
  under: {},
};

describe("addGroup", () => {
  it("adds a named group and gives it back", () => {
    const { groups, id } = addGroup(empty, "Work");
    expect(id).not.toBeNull();
    expect(groups.order).toEqual([id]);
    expect(groups.names[id as string]).toBe("Work");
  });

  it("does not reuse an id that is taken", () => {
    const first = addGroup(empty, "Work");
    const second = addGroup(first.groups, "Toys");
    expect(second.id).not.toBe(first.id);
    expect(second.groups.order).toEqual([first.id, second.id]);
  });

  it("refuses a blank name rather than creating a nameless group", () => {
    expect(addGroup(empty, "   ").id).toBeNull();
    expect(addGroup(empty, "   ").groups).toEqual(empty);
  });
});

describe("renameGroup", () => {
  it("renames the one asked for and no other", () => {
    const renamed = renameGroup(withTwo, "g1", "Day job");
    expect(renamed.names).toEqual({ g1: "Day job", g2: "Toys" });
  });

  it("ignores a blank name", () => {
    expect(renameGroup(withTwo, "g1", "  ")).toEqual(withTwo);
  });

  it("ignores a group that does not exist", () => {
    expect(renameGroup(withTwo, "nope", "Whatever")).toEqual(withTwo);
  });
});

describe("removeGroup", () => {
  it("takes the group out of the order", () => {
    expect(removeGroup(withTwo, "g1").order).toEqual(["g2"]);
  });

  it("returns its repositories to the ungrouped bucket rather than losing them", () => {
    const left = removeGroup(withTwo, "g1");
    expect(left.of["/w/alpha"]).toBeUndefined();
    expect(left.of["/w/gamma"]).toBe("g2");
  });

  // Delete this group went at once, name, nesting and members, with nothing to take it back.
  it("is asked about with its name and what moves where", () => {
    expect(removalQuestion(withTwo, "g1")).toBe(
      "Delete the group Work? Its 2 repositories go to Ungrouped. Nothing on disk changes.",
    );
    const nested = { ...withTwo, under: { g2: "g1" } };
    expect(removalQuestion(nested, "g1")).toContain("Its 1 group moves up a level.");
  });
});

describe("assign", () => {
  it("moves a repository into a group", () => {
    expect(assign(withTwo, "/w/gamma", "g1").of["/w/gamma"]).toBe("g1");
  });

  it("takes it out of every group when the target is the ungrouped bucket", () => {
    expect(assign(withTwo, "/w/alpha", UNGROUPED).of["/w/alpha"]).toBeUndefined();
  });

  it("ignores a group that does not exist, rather than hiding the repository", () => {
    expect(assign(withTwo, "/w/alpha", "nope")).toEqual(withTwo);
  });
});

describe("groupRows", () => {
  const roots = ["/w/alpha", "/w/beta", "/w/gamma", "/w/loose"];

  it("lists each group with the repositories in it, in the group order", () => {
    const rows = groupRows(withTwo, roots, new Set());
    expect(rows.filter((row) => row.kind === "group").map((row) => row.name)).toEqual([
      "Work",
      "Toys",
      "Ungrouped",
    ]);
  });

  it("puts a repository nobody claimed in the ungrouped bucket", () => {
    const rows = groupRows(withTwo, roots, new Set());
    const at = rows.findIndex((row) => row.kind === "group" && row.id === UNGROUPED);
    expect(rows[at + 1]).toEqual({ kind: "repo", root: "/w/loose", group: UNGROUPED, depth: 1 });
  });

  it("leaves out the ungrouped heading when everything is claimed", () => {
    const rows = groupRows(withTwo, ["/w/alpha", "/w/gamma"], new Set());
    expect(rows.some((row) => row.kind === "group" && row.id === UNGROUPED)).toBe(false);
  });

  it("shows an empty group so it can be dropped into", () => {
    const rows = groupRows(withTwo, ["/w/gamma"], new Set());
    expect(rows.some((row) => row.kind === "group" && row.name === "Work")).toBe(true);
  });

  it("hides the repositories of a collapsed group but keeps its heading", () => {
    const rows = groupRows(withTwo, roots, new Set(["g1"]));
    expect(rows.some((row) => row.kind === "group" && row.id === "g1")).toBe(true);
    expect(rows.some((row) => row.kind === "repo" && row.root === "/w/alpha")).toBe(false);
  });

  it("shows no headings at all when there are no groups", () => {
    const rows = groupRows(empty, roots, new Set());
    expect(rows.every((row) => row.kind === "repo")).toBe(true);
  });

  it("keeps the given repository order inside a group", () => {
    const rows = groupRows(withTwo, ["/w/beta", "/w/alpha"], new Set());
    const inWork = rows.filter((row) => row.kind === "repo").map((row) => row.root);
    expect(inWork).toEqual(["/w/beta", "/w/alpha"]);
  });

  it("says on each heading whether its rows are drawn", () => {
    const rows = groupRows(withTwo, roots, new Set(["g1"]));
    const open = rows.flatMap((row) => (row.kind === "group" ? [[row.id, row.open]] : []));
    expect(open).toEqual([
      ["g1", false],
      ["g2", true],
      [UNGROUPED, true],
    ]);
  });
});

describe("groupRows while filtering", () => {
  const everyFolded = new Set(["g1", "g2", UNGROUPED]);
  const ids = (rows: ReturnType<typeof groupRows>) =>
    rows.map((row) => (row.kind !== "repo" ? `g:${row.id}` : row.root));

  it("shows the matches of a folded group", () => {
    const folds = shownFolds(everyFolded, "alp", NO_FILTER_FOLDS);
    expect(ids(groupRows(withTwo, ["/w/alpha"], folds, true))).toEqual(["g:g1", "/w/alpha"]);
  });

  it("leaves out a group with no match", () => {
    const rows = groupRows(withTwo, ["/w/gamma"], new Set(), true);
    expect(ids(rows)).toEqual(["g:g2", "/w/gamma"]);
  });

  it("keeps a parent whose only match is in a group inside it", () => {
    const nested: RepoGroups = { ...withTwo, under: { g2: "g1" } };
    const rows = groupRows(nested, ["/w/gamma"], new Set(), true);
    expect(ids(rows)).toEqual(["g:g1", "g:g2", "/w/gamma"]);
  });

  it("draws the folds from before once the filter is cleared", () => {
    const folds = shownFolds(everyFolded, "", NO_FILTER_FOLDS);
    const every = ["/w/alpha", "/w/gamma", "/w/loose"];
    expect(ids(groupRows(withTwo, every, folds))).toEqual(["g:g1", "g:g2", `g:${UNGROUPED}`]);
  });
});

describe("mergeGroups", () => {
  it("survives a store that holds nothing", () => {
    expect(mergeGroups(null)).toEqual(empty);
  });

  it("survives a store that holds the wrong shape", () => {
    expect(mergeGroups({ order: "nonsense", names: 7 })).toEqual(empty);
  });

  it("drops an assignment pointing at a group that is gone", () => {
    const stored = {
      order: ["g1"],
      names: { g1: "Work" },
      of: { "/w/a": "g1", "/w/b": "ghost" },
    };
    expect(mergeGroups(stored).of).toEqual({ "/w/a": "g1" });
  });

  it("drops an order entry with no name behind it", () => {
    const stored = { order: ["g1", "ghost"], names: { g1: "Work" }, of: {} };
    expect(mergeGroups(stored).order).toEqual(["g1"]);
  });
});

describe("showsFilter", () => {
  it("shows the box once there are two repositories to choose between", () => {
    expect(showsFilter(1, "")).toBe(false);
    expect(showsFilter(2, "")).toBe(true);
  });

  // Removing the match left one repository hidden by a filter with no box to clear it in.
  it("keeps the box while a filter is typed, however few repositories are left", () => {
    expect(showsFilter(1, "beta")).toBe(true);
    expect(showsFilter(0, "beta")).toBe(true);
  });
});

describe("backendRows", () => {
  const where = new Map([
    ["/w/alpha", "local"],
    ["/w/beta", "WSL: Ubuntu"],
    ["/w/gamma", "local"],
    ["/w/loose", "WSL: Ubuntu"],
  ]);
  const roots = ["/w/beta", "/w/alpha", "/w/gamma", "/w/loose"];
  const ids = (rows: ReturnType<typeof backendRows>) =>
    rows.map((row) => (row.kind === "repo" ? `${row.depth}${row.root}` : `${row.depth}${row.kind}:${row.id}`));

  it("is the plain list while every repository lives on one backend", () => {
    const local = new Map(roots.map((root) => [root, "local"]));
    expect(backendRows(withTwo, roots, local, new Set())).toEqual(groupRows(withTwo, roots, new Set()));
  });

  it("puts Local first and each backend's repositories under it", () => {
    const rows = backendRows(empty, roots, where, new Set());
    expect(ids(rows)).toEqual([
      "0backend:backend:local",
      "1/w/alpha",
      "1/w/gamma",
      "0backend:backend:WSL: Ubuntu",
      "1/w/beta",
      "1/w/loose",
    ]);
    expect(rows[0]).toMatchObject({ name: "Local", count: 2, open: true });
  });

  it("shows a group under each backend that has a repository in it", () => {
    const rows = backendRows(withTwo, roots, where, new Set());
    expect(ids(rows)).toEqual([
      "0backend:backend:local",
      "1group:g1",
      "2/w/alpha",
      "1group:g2",
      "2/w/gamma",
      "0backend:backend:WSL: Ubuntu",
      "1group:g1",
      "2/w/beta",
      "1group:",
      "2/w/loose",
    ]);
  });

  it("keys a group shown twice apart", () => {
    const rows = backendRows(withTwo, roots, where, new Set());
    const keys = rows.flatMap((row) => (row.kind === "group" ? [rowKey(row)] : []));
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("shows an empty group under the first backend so it can be dropped into", () => {
    const withEmpty: RepoGroups = { ...withTwo, order: [...withTwo.order, "g3"], names: { ...withTwo.names, g3: "New" } };
    const rows = backendRows(withEmpty, roots, where, new Set());
    expect(rows.filter((row) => row.kind === "group" && row.id === "g3")).toHaveLength(1);
    expect(ids(rows).indexOf("1group:g3")).toBeLessThan(ids(rows).indexOf("0backend:backend:WSL: Ubuntu"));
  });

  it("hides what a folded backend holds", () => {
    const rows = backendRows(empty, roots, where, new Set(["backend:local"]));
    expect(ids(rows)).toEqual(["0backend:backend:local", "0backend:backend:WSL: Ubuntu", "1/w/beta", "1/w/loose"]);
    expect(rows[0]).toMatchObject({ count: 2, open: false });
  });

  it("leaves out a backend with no match while filtering", () => {
    const rows = backendRows(empty, ["/w/beta"], where, new Set(), true);
    expect(ids(rows)).toEqual(["0backend:backend:WSL: Ubuntu", "1/w/beta"]);
  });
});
