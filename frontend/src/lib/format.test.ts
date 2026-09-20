import { describe, expect, it } from "vitest";
import {
  capsules,
  formatCommitDate,
  headLabel,
  refLabels,
  relativeDate,
  shortOid,
  splitBranches,
  type RefLabel,
} from "./format";
import type { Tag } from "./ipc";
import type { Branch, Head } from "./ipc";

function branch(name: string, kind: Branch["kind"], isHead = false): Branch {
  return {
    name,
    fullName: `refs/${kind}/${name}`,
    kind,
    oid: "a".repeat(40),
    isHead,
    upstream: null,
    ahead: 0,
    behind: 0,
  };
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
    {
      name: "main",
      fullName: "refs/heads/main",
      kind: "local",
      oid: "a".repeat(40),
      isHead: true,
      upstream: null,
      ahead: 0,
      behind: 0,
    },
    {
      name: "dev",
      fullName: "refs/heads/dev",
      kind: "local",
      oid: "b".repeat(40),
      isHead: false,
      upstream: null,
      ahead: 0,
      behind: 0,
    },
    {
      name: "origin/main",
      fullName: "refs/remotes/origin/main",
      kind: "remote",
      oid: "a".repeat(40),
      isHead: false,
      upstream: null,
      ahead: 0,
      behind: 0,
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

describe("relativeDate", () => {
  const NOW = Date.UTC(2026, 0, 15, 12, 0, 0) / 1000;
  const at = (seconds: number) => relativeDate(NOW - seconds, 0, NOW);

  it("calls the last minute just now", () => {
    expect(at(30)).toBe("just now");
  });

  it("counts whole minutes", () => {
    expect(at(5 * 60)).toBe("5 minutes ago");
  });

  it("uses the singular for one of anything", () => {
    expect(at(60)).toBe("1 minute ago");
    expect(at(3600)).toBe("1 hour ago");
  });

  it("counts hours, then days, then months, then years", () => {
    expect(at(5 * 3600)).toBe("5 hours ago");
    expect(at(3 * 86400)).toBe("3 days ago");
    expect(at(70 * 86400)).toBe("2 months ago");
    expect(at(800 * 86400)).toBe("2 years ago");
  });

  it("does not claim a future commit happened in the past", () => {
    expect(relativeDate(NOW + 600, 0, NOW)).toBe("just now");
  });
});

describe("capsules", () => {
  const many = (n: number): RefLabel[] =>
    Array.from({ length: n }, (_, i) => ({ text: `branch-${i}`, kind: "local" as const }));

  it("shows every label when there are few", () => {
    const { shown, hidden } = capsules(many(2), 3);
    expect(shown).toHaveLength(2);
    expect(hidden).toEqual([]);
  });

  it("keeps the first labels and hides the rest", () => {
    const { shown, hidden } = capsules(many(10), 3);
    expect(shown.map((l) => l.text)).toEqual(["branch-0", "branch-1", "branch-2"]);
    expect(hidden).toHaveLength(7);
  });

  it("reports the hidden ones so a tooltip can name them", () => {
    expect(capsules(many(5), 2).hidden.map((l) => l.text)).toEqual([
      "branch-2",
      "branch-3",
      "branch-4",
    ]);
  });

  it("shows nothing when there is no room at all", () => {
    expect(capsules(many(3), 0).shown).toEqual([]);
  });

  it("copes with an empty list", () => {
    expect(capsules([], 3)).toEqual({ shown: [], hidden: [] });
  });
});
