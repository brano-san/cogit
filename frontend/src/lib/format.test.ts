import { describe, expect, it } from "vitest";
import { displayDate, smartDate, capsules, dateTooltip, headLabel, refLabelKey, refLabels, relativeDate, shortOid, splitBranches, type RefLabel, fileFormat } from "./format";
import type { Tag } from "./ipc";
import type { Branch, Head, WorktreeEntry } from "./ipc";

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
    { name: "v1.0", fullName: "refs/tags/v1.0", oid: "a".repeat(40), isAnnotated: false, pointsToCommit: true },
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

  it("gives a branch and a tag of the same name on one commit different keys", () => {
    const release: Branch = {
      name: "v1.0",
      fullName: "refs/heads/v1.0",
      kind: "local",
      oid: "a".repeat(40),
      isHead: false,
      upstream: null,
      ahead: 0,
      behind: 0,
    };
    const labels = refLabels([...branches, release], tags, head).get("a".repeat(40)) ?? [];
    const keys = labels.map(refLabelKey);
    expect(labels.filter((l) => l.text === "v1.0")).toHaveLength(2);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("does not mark anything as HEAD when the head is detached", () => {
    const detached: Head = { kind: "detached", oid: "a".repeat(40) };
    const kinds = refLabels(branches, tags, detached)
      .get("a".repeat(40))
      ?.map((l) => l.kind);
    expect(kinds).not.toContain("head");
  });
});

describe("refLabels with the upstream on the same commit", () => {
  const A = "a".repeat(40);
  const B = "b".repeat(40);
  const local = (name: string, oid: string, upstream: string | null): Branch => ({
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid,
    isHead: false,
    upstream,
    ahead: 0,
    behind: 0,
  });
  const remote = (name: string, oid: string): Branch => ({
    name,
    fullName: `refs/remotes/${name}`,
    kind: "remote",
    oid,
    isHead: false,
    upstream: null,
    ahead: 0,
    behind: 0,
  });
  const at = (list: Branch[], oid = A, head: Head | null = null) =>
    refLabels(list, [], head).get(oid) ?? [];

  it("draws a branch and its upstream as one label", () => {
    const labels = at([local("feature/x", A, "origin/feature/x"), remote("origin/feature/x", A)]);

    expect(labels).toHaveLength(1);
    expect(labels[0]).toMatchObject({
      text: "origin=feature/x",
      kind: "local",
      remotes: ["origin"],
      name: "feature/x",
    });
  });

  it("names both refs in the tooltip of the joined label", () => {
    const [label] = at([local("dev", A, "origin/dev"), remote("origin/dev", A)]);

    expect(label?.title).toBe("dev\norigin/dev");
  });

  it("keeps two labels once the branch and its upstream have diverged", () => {
    const list = [local("dev", A, "origin/dev"), remote("origin/dev", B)];

    expect(at(list).map((l) => l.text)).toEqual(["dev"]);
    expect(at(list, B).map((l) => l.text)).toEqual(["origin/dev"]);
  });

  it("puts every remote with the same branch on that commit into the one label", () => {
    const labels = at([
      remote("upstream/dev", A),
      local("dev", A, "origin/dev"),
      remote("origin/dev", A),
      remote("fork/dev", A),
    ]);

    expect(labels.map((l) => l.text)).toEqual(["origin,fork,upstream=dev"]);
    expect(labels[0]?.remotes).toEqual(["origin", "fork", "upstream"]);
  });

  it("leaves a branch without an upstream apart from a remote namesake", () => {
    expect(at([local("dev", A, null), remote("origin/dev", A)]).map((l) => l.text)).toEqual([
      "dev",
      "origin/dev",
    ]);
  });

  it("leaves an upstream of another name as its own label", () => {
    expect(at([local("main", A, "origin/master"), remote("origin/master", A)]).map((l) => l.text)).toEqual([
      "main",
      "origin/master",
    ]);
  });

  it("keeps the checked-out branch drawn as HEAD when it is joined", () => {
    const head: Head = { kind: "branch", name: "main", oid: A };
    const [label] = at([local("main", A, "origin/main"), remote("origin/main", A)], A, head);

    expect(label).toMatchObject({ kind: "head", text: "origin=main" });
  });

  it("keeps a remote branch that no local branch joined", () => {
    const labels = at([local("dev", A, "origin/dev"), remote("origin/dev", A), remote("origin/other", A)]);

    expect(labels.map((l) => l.text)).toEqual(["origin=dev", "origin/other"]);
  });
});

describe("refLabels for stashes", () => {
  const S = "5".repeat(40);
  const stash = (index: number, oid: string, message: string) => ({ index, oid, message, timestamp: 0 });
  const tag: Tag = { name: "v1", fullName: "refs/tags/v1", oid: S, isAnnotated: false, pointsToCommit: true };

  it("labels a stash commit with its name and keeps the message for the tooltip", () => {
    const [label] = refLabels([], [], null, { stashes: [stash(0, S, "On main: halfway")] }).get(S) ?? [];

    expect(label).toEqual({ text: "stash@{0}", kind: "stash", title: "stash@{0}\nOn main: halfway" });
  });

  it("puts a stash label after the branch and tag labels", () => {
    const labels = refLabels([], [tag], null, { stashes: [stash(2, S, "wip")] }).get(S) ?? [];

    expect(labels.map((l) => l.kind)).toEqual(["tag", "stash"]);
  });
});

describe("refLabels for branches held by worktrees", () => {
  const A = "a".repeat(40);
  const dev: Branch = {
    name: "dev",
    fullName: "refs/heads/dev",
    kind: "local",
    oid: A,
    isHead: false,
    upstream: null,
    ahead: 0,
    behind: 0,
  };
  const tree = (over: Partial<WorktreeEntry>): WorktreeEntry => ({
    path: "C:/work/dev",
    name: "dev",
    branch: "dev",
    head: A,
    isMain: false,
    isCurrent: false,
    locked: null,
    missing: false,
    dirty: false,
    hasSubmodules: false,
    ...over,
  });
  const label = (entry: WorktreeEntry) => refLabels([dev], [], null, { worktrees: [entry] }).get(A)?.[0];

  it("marks a branch checked out in another worktree with where it is and how it stands", () => {
    expect(label(tree({ dirty: true }))).toMatchObject({
      text: "dev",
      worktree: { path: "C:/work/dev", state: "modified" },
      title: "dev\nChecked out in worktree C:/work/dev (modified)",
    });
    expect(label(tree({}))?.worktree?.state).toBe("clean");
    expect(label(tree({ missing: true }))?.worktree?.state).toBe("missing");
  });

  it("leaves unmarked the branch of the worktree the panels show", () => {
    expect(label(tree({ isCurrent: true }))?.worktree).toBeUndefined();
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

describe("dateTooltip", () => {
  it("always gives the exact time, whatever the display format is", () => {
    expect(dateTooltip(Date.UTC(2026, 0, 15, 12, 0, 0) / 1000, 0)).toContain("2026-01-15");
  });

  it("keeps the author's offset so the tooltip is not machine-dependent", () => {
    const at = Date.UTC(2026, 0, 15, 12, 0, 0) / 1000;
    expect(dateTooltip(at, 180)).toContain("15:00");
    expect(dateTooltip(at, 180)).toContain("+03:00");
  });

  it("shows a negative offset with its sign", () => {
    expect(dateTooltip(Date.UTC(2026, 0, 15, 12, 0, 0) / 1000, -300)).toContain("-05:00");
  });

  it("writes a zero offset as +00:00", () => {
    expect(dateTooltip(0, 0)).toContain("+00:00");
  });
});

describe("smartDate", () => {
  const DAY = 86_400;
  // Thursday 2026-09-17 12:00:00 UTC
  const now = Date.UTC(2026, 8, 17, 12, 0, 0) / 1000;

  it("calls the same calendar day today", () => {
    expect(smartDate(now - 3600, 0, now)).toBe("today");
  });

  it("calls the day before yesterday", () => {
    expect(smartDate(now - DAY, 0, now)).toBe("yesterday");
  });

  it("names the weekday inside the last week", () => {
    expect(smartDate(now - 3 * DAY, 0, now)).toBe("Monday");
  });

  it("still names the weekday six days back", () => {
    expect(smartDate(now - 6 * DAY, 0, now)).toBe("Friday");
  });

  it("falls back to a date once a week has passed", () => {
    expect(smartDate(now - 8 * DAY, 0, now)).toBe("09-09-26");
  });

  it("writes a date for anything in the future, which is clock skew", () => {
    expect(smartDate(now + 3 * DAY, 0, now)).toBe("20-09-26");
  });

  it("reads the day boundary in the commit's own timezone", () => {
    // 23:30 in UTC+3 is still the same day there, though it is 20:30 UTC.
    const late = Date.UTC(2026, 8, 17, 20, 30, 0) / 1000;
    expect(smartDate(late, 180, now)).toBe("today");
  });
});

describe("displayDate", () => {
  const now = Date.UTC(2026, 8, 17, 12, 0, 0) / 1000;
  const yesterday = now - 86_400;

  it("shows the smart form alone", () => {
    expect(displayDate(yesterday, 0, now, "smart")).toBe("yesterday");
  });

  it("shows the elapsed form alone", () => {
    expect(displayDate(yesterday, 0, now, "relative")).toBe("1 day ago");
  });

  it("shows both, smart first", () => {
    expect(displayDate(yesterday, 0, now, "both")).toBe("yesterday · 1 day ago");
  });
});

// The status bar was never told either, so every file read "UTF-8 • LF", CRLF ones too.
describe("fileFormat", () => {
  const text = (eol: "lf" | "crlf" | "mixed" | "none", lossyEncoding = false) =>
    ({ kind: "text", eol: { old: "lf", new: eol, normalized: false }, lossyEncoding }) as never;

  it("names the line ending of the file as it is now", () => {
    expect(fileFormat(text("crlf"))).toEqual({ encoding: "UTF-8", lineEnding: "CRLF" });
    expect(fileFormat(text("mixed")).lineEnding).toBe("Mixed");
  });

  it("says so when the file is not UTF-8", () => {
    expect(fileFormat(text("lf", true)).encoding).toBe("Not UTF-8");
  });

  it("says nothing for what is not a text diff", () => {
    expect(fileFormat(null)).toEqual({});
    expect(fileFormat({ kind: "binary" } as never)).toEqual({});
  });
});
