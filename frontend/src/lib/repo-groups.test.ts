import { describe, expect, it } from "vitest";
import {
  UNGROUPED,
  addGroup,
  assign,
  groupRows,
  mergeGroups,
  removeGroup,
  renameGroup,
  type RepoGroups,
} from "./repo-groups";

const empty: RepoGroups = { order: [], names: {}, of: {} };

const withTwo: RepoGroups = {
  order: ["g1", "g2"],
  names: { g1: "Work", g2: "Toys" },
  of: { "/w/alpha": "g1", "/w/beta": "g1", "/w/gamma": "g2" },
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
    expect(rows[at + 1]).toEqual({ kind: "repo", root: "/w/loose", group: UNGROUPED });
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
});

describe("mergeGroups", () => {
  it("survives a store that holds nothing", () => {
    expect(mergeGroups(null)).toEqual(empty);
  });

  it("survives a store that holds the wrong shape", () => {
    expect(mergeGroups({ order: "nonsense", names: 7 })).toEqual(empty);
  });

  it("drops an assignment pointing at a group that is gone", () => {
    const stored = { order: ["g1"], names: { g1: "Work" }, of: { "/w/a": "g1", "/w/b": "ghost" } };
    expect(mergeGroups(stored).of).toEqual({ "/w/a": "g1" });
  });

  it("drops an order entry with no name behind it", () => {
    const stored = { order: ["g1", "ghost"], names: { g1: "Work" }, of: {} };
    expect(mergeGroups(stored).order).toEqual(["g1"]);
  });
});
