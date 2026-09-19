import { describe, expect, it } from "vitest";
import { headLabel, splitBranches } from "./format";
import type { Branch, Head } from "./ipc";

function branch(name: string, kind: Branch["kind"], isHead = false): Branch {
  return { name, fullName: `refs/${kind}/${name}`, kind, oid: "a".repeat(40), isHead };
}

describe("headLabel", () => {
  it("shows the branch name when HEAD follows a branch", () => {
    const head: Head = { kind: "branch", name: "main", oid: "4ec4813".padEnd(40, "0") };
    expect(headLabel(head)).toBe("main");
  });

  it("shows a shortened commit when HEAD is detached", () => {
    const head: Head = { kind: "detached", oid: "4ec48139abcdef0123456789abcdef0123456789" };
    expect(headLabel(head)).toBe("detached at 4ec4813");
  });

  it("marks an unborn branch rather than pretending it exists", () => {
    // A fresh repository must not read as if it were on a normal branch.
    const head: Head = { kind: "unborn", name: "main" };
    expect(headLabel(head)).toBe("main (unborn)");
  });

  it("falls back to a dash when no repository is open", () => {
    expect(headLabel(null)).toBe("—");
  });
});

describe("splitBranches", () => {
  it("separates local from remote-tracking branches", () => {
    const all = [
      branch("main", "local", true),
      branch("origin/main", "remote"),
      branch("dev", "local"),
    ];
    const { local, remote } = splitBranches(all);
    expect(local.map((b) => b.name)).toEqual(["main", "dev"]);
    expect(remote.map((b) => b.name)).toEqual(["origin/main"]);
  });

  it("handles a repository with no branches at all", () => {
    const { local, remote } = splitBranches([]);
    expect(local).toEqual([]);
    expect(remote).toEqual([]);
  });

  it("preserves the order the backend supplied", () => {
    // The backend sorts; re-sorting here would be a second source of truth.
    const all = [branch("a", "local"), branch("b", "local"), branch("c", "local")];
    expect(splitBranches(all).local.map((b) => b.name)).toEqual(["a", "b", "c"]);
  });
});
