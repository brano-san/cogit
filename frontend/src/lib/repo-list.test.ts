import { describe, expect, it } from "vitest";
import type { RepoOverview } from "./ipc";
import {
  EMPTY_LIST,
  folderName,
  forget,
  listedName,
  listedRepos,
  markClosed,
  markOpened,
  readRepoList,
  rename,
  togglePin,
} from "./repo-list";

function overview(root: string, name = folderName(root)): RepoOverview {
  return {
    repo: root.length,
    name,
    root,
    branch: "main",
    ahead: 0,
    behind: 0,
    dirty: false,
    missing: false,
    state: { kind: "clean" },
  };
}

describe("readRepoList", () => {
  it("keeps what it recognises and drops the rest", () => {
    const list = readRepoList({ closed: ["/a", 3, "/a"], names: { "/a": "Alpha", "/b": 7, "/c": " " }, pinned: "no" });
    expect(list).toEqual({ closed: ["/a"], names: { "/a": "Alpha" }, pinned: [] });
  });

  it("starts empty from nothing", () => {
    expect(readRepoList(null)).toEqual(EMPTY_LIST);
  });
});

describe("closing and opening", () => {
  it("remembers a closed root once, and forgets it on opening", () => {
    const closed = markClosed(EMPTY_LIST, "/a");
    expect(markClosed(closed, "/a")).toBe(closed);
    expect(markOpened(closed, "/a").closed).toEqual([]);
    expect(markOpened(EMPTY_LIST, "/a")).toBe(EMPTY_LIST);
  });

  it("forgets name and pin along with the row", () => {
    const list = togglePin(rename(markClosed(EMPTY_LIST, "/a"), "/a", "Alpha"), "/a");
    expect(forget(list, "/a")).toEqual(EMPTY_LIST);
  });
});

describe("rename", () => {
  it("shows the given name and gives the folder name back when cleared", () => {
    const named = rename(EMPTY_LIST, "D:/work/cogit", "  Client  ");
    expect(named.names).toEqual({ "D:/work/cogit": "Client" });
    expect(rename(named, "D:/work/cogit", "").names).toEqual({});
    expect(rename(named, "D:/work/cogit", "cogit").names).toEqual({});
  });

  // Rename… on a closed row put the whole path in the field, and Enter kept it as the name.
  it("starts from the name the row shows, a closed row's folder name included", () => {
    expect(listedName(EMPTY_LIST, "D:/work/foo", null)).toBe("foo");
    expect(listedName(EMPTY_LIST, "D:/work/foo", overview("D:/work/foo", "Foo"))).toBe("Foo");
    expect(listedName(rename(EMPTY_LIST, "D:/work/foo", "Client"), "D:/work/foo", null)).toBe("Client");
  });
});

describe("listedRepos", () => {
  it("lists closed repositories next to the open ones, by name", () => {
    const rows = listedRepos([overview("/w/beta")], markClosed(EMPTY_LIST, "/w/alpha"));
    expect(rows.map((row) => [row.name, row.overview !== null])).toEqual([
      ["alpha", false],
      ["beta", true],
    ]);
  });

  it("shows an open repository once even when a stale record calls it closed", () => {
    const rows = listedRepos([overview("/w/alpha")], markClosed(EMPTY_LIST, "/w/alpha"));
    expect(rows).toHaveLength(1);
    expect(rows[0]!.overview).not.toBeNull();
  });

  it("puts pinned rows first, in the order they were pinned, under their own names", () => {
    let list = togglePin(togglePin(EMPTY_LIST, "/w/zeta"), "/w/gamma");
    list = rename(list, "/w/gamma", "Gamma Ray");
    const rows = listedRepos([overview("/w/alpha"), overview("/w/gamma"), overview("/w/zeta")], list);
    expect(rows.map((row) => row.name)).toEqual(["zeta", "Gamma Ray", "alpha"]);
    expect(rows.map((row) => row.pinned)).toEqual([true, true, false]);
  });
});
