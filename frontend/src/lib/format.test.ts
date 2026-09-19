import { describe, expect, it } from "vitest";
import { formatCommitDate, headLabel, refLabels, shortOid, splitBranches } from "./format";
import type { Tag } from "./ipc";
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
    const all = [branch("a", "local"), branch("b", "local"), branch("c", "local")];
    expect(splitBranches(all).local.map((b) => b.name)).toEqual(["a", "b", "c"]);
  });
});

describe("shortOid", () => {
  it("shortens to seven characters", () => {
    expect(shortOid("4ec48139abcdef0123456789abcdef0123456789")).toBe("4ec4813");
  });

  it("leaves an already short value alone", () => {
    expect(shortOid("abc")).toBe("abc");
  });
});

describe("formatCommitDate", () => {
  it("renders the time the author saw, not the reader's", () => {
    expect(formatCommitDate(1_767_225_600, 180)).toBe("2026-01-01 03:00");
  });

  it("handles negative offsets", () => {
    expect(formatCommitDate(1_767_225_600, -300)).toBe("2025-12-31 19:00");
  });

  it("treats a zero offset as UTC", () => {
    expect(formatCommitDate(1_767_225_600, 0)).toBe("2026-01-01 00:00");
  });
});

describe("refLabels", () => {
  const head: Head = { kind: "branch", name: "main", oid: "a".repeat(40) };
  const branches: Branch[] = [
    { name: "main", fullName: "refs/heads/main", kind: "local", oid: "a".repeat(40), isHead: true },
    { name: "dev", fullName: "refs/heads/dev", kind: "local", oid: "b".repeat(40), isHead: false },
    {
      name: "origin/main",
      fullName: "refs/remotes/origin/main",
      kind: "remote",
      oid: "a".repeat(40),
      isHead: false,
    },
  ];
  const tags: Tag[] = [
    { name: "v1.0", fullName: "refs/tags/v1.0", oid: "a".repeat(40), isAnnotated: false },
  ];

  it("groups every ref by the commit it points at", () => {
    const map = refLabels(branches, tags, head);
    expect(map.get("a".repeat(40))?.length).toBe(3);
    expect(map.get("b".repeat(40))?.length).toBe(1);
  });

  it("marks the checked-out branch so it can be drawn as HEAD", () => {
    const map = refLabels(branches, tags, head);
    const main = map.get("a".repeat(40))?.find((l) => l.text === "main");
    expect(main?.kind).toBe("head");
  });

  it("keeps remote branches apart from local ones", () => {
    const map = refLabels(branches, tags, head);
    const remote = map.get("a".repeat(40))?.find((l) => l.text === "origin/main");
    expect(remote?.kind).toBe("remote");
  });

  it("orders labels HEAD first, then local, remote and tags", () => {
    const kinds = refLabels(branches, tags, head)
      .get("a".repeat(40))
      ?.map((l) => l.kind);
    expect(kinds).toEqual(["head", "remote", "tag"]);
  });

  it("returns nothing for a commit with no refs", () => {
    expect(refLabels(branches, tags, head).get("c".repeat(40))).toBeUndefined();
  });

  it("does not mark anything as HEAD when the head is detached", () => {
    const detached: Head = { kind: "detached", oid: "a".repeat(40) };
    const kinds = refLabels(branches, tags, detached)
      .get("a".repeat(40))
      ?.map((l) => l.kind);
    expect(kinds).not.toContain("head");
  });
});
