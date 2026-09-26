import { describe, expect, it } from "vitest";
import {
  canPull,
  freshOverview,
  moduleSync,
  REMOTE_AHEAD,
  rowSync,
  summaryPulse,
  syncTooltip,
  UNKNOWN_PULL,
} from "./repo-sync";
import type { Branch, RepoOverview, RepoSummary } from "$lib/ipc";
import type { RepoPulse } from "$lib/ipc/bindings";

const overview = (over: Partial<RepoOverview> = {}): RepoOverview => ({
  repo: 1 as never,
  name: "a",
  root: "/a",
  branch: "main",
  ahead: 1,
  behind: 0,
  dirty: true,
  missing: false,
  state: { kind: "clean" } as never,
  ...over,
});

const pulse = (over: Partial<RepoPulse> = {}): RepoPulse => ({
  missing: false,
  branch: "main",
  tracked: true,
  ahead: 0,
  behind: 3,
  dirty: false,
  ...over,
});

describe("the marks of a repository row", () => {
  it("reads the repository on screen from its full status", () => {
    const sync = rowSync({ overview: overview(), owned: true, pulse: pulse(), fetchFailed: false });
    expect(sync).toMatchObject({ dirty: true, ahead: 1, behind: 0 });
  });

  it("reads any other from its pulse, which is fresher than the list", () => {
    const sync = rowSync({ overview: overview(), owned: false, pulse: pulse(), fetchFailed: false });
    expect(sync).toMatchObject({ dirty: false, ahead: 0, behind: 3 });
  });

  it("claims nothing about a closed row not read yet", () => {
    expect(rowSync({ overview: null, owned: false, pulse: undefined, fetchFailed: false })).toEqual({
      dirty: null,
      ahead: 0,
      behind: 0,
      unknown: false,
      remoteAhead: false,
      missing: false,
      branch: null,
    });
  });

  // After `git switch other` in a repository the panels do not show, the row kept the old
  // branch beside the new branch's arrows; a closed row named none at all.
  it("names the branch its pulse read, closed rows included", () => {
    const other = pulse({ branch: "other" });
    expect(rowSync({ overview: overview(), owned: false, pulse: other, fetchFailed: false }).branch).toBe("other");
    expect(rowSync({ overview: null, owned: false, pulse: other, fetchFailed: false }).branch).toBe("other");
    expect(rowSync({ overview: overview(), owned: true, pulse: other, fetchFailed: false }).branch).toBe("main");
  });

  it("offers a pull when the server moved on though the tracking ref is level", () => {
    const sync = rowSync({
      overview: null,
      owned: false,
      pulse: pulse({ behind: 0 }),
      fetchFailed: false,
      remoteAhead: true,
    });
    expect(canPull(sync)).toBe(true);
    expect(syncTooltip(sync)).toContain(REMOTE_AHEAD);
  });

  it("claims no pull from a probe that failed", () => {
    const sync = rowSync({
      overview: null,
      owned: false,
      pulse: pulse({ behind: 0 }),
      fetchFailed: true,
      remoteAhead: true,
    });
    expect(canPull(sync)).toBe(false);
    expect(sync.unknown).toBe(true);
  });

  it("says a closed row's folder is gone", () => {
    const sync = rowSync({
      overview: null,
      owned: false,
      pulse: pulse({ missing: true }),
      fetchFailed: false,
    });
    expect(sync.missing).toBe(true);
    expect(sync.dirty).toBeNull();
  });

  it("marks pull as unknown after a failed fetch, and says so in the tooltip", () => {
    const sync = rowSync({ overview: null, owned: false, pulse: pulse(), fetchFailed: true });
    expect(sync.unknown).toBe(true);
    expect(syncTooltip(sync)).toContain(UNKNOWN_PULL);
  });

  it("has no tooltip for a clean row in step with its upstream", () => {
    const sync = rowSync({ overview: null, owned: false, pulse: pulse({ behind: 0 }), fetchFailed: false });
    expect(syncTooltip(sync)).toBe("");
  });
});

describe("the row of the repository on screen", () => {
  const local = (name: string, over: Partial<Branch> = {}): Branch => ({
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: "a".repeat(40),
    isHead: false,
    upstream: `origin/${name}`,
    ahead: 0,
    behind: 0,
    ...over,
  });
  const summary = (over: Partial<RepoSummary> = {}): RepoSummary =>
    ({
      repo: 1,
      root: "/a",
      name: "a",
      isBare: false,
      head: { kind: "branch", name: "main", oid: "a".repeat(40) },
      branches: [local("main")],
      tags: [],
      status: { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 },
      state: { kind: "clean" },
      indexLock: null,
      tagGroupSeparator: "/",
      ...over,
    }) as RepoSummary;
  const opened = overview({ dirty: false, ahead: 0, behind: 0, branch: "main" });

  // The list is read again on open, fetch and the like, not after a commit, a checkout or
  // an edit on disk: the marks of the active row froze at the moment it was opened.
  it("takes the changes, the branch and its ahead and behind from the panels' status", () => {
    const now = summary({
      head: { kind: "branch", name: "feature", oid: "b".repeat(40) },
      branches: [local("main"), local("feature", { isHead: true, ahead: 2, behind: 1 })],
      status: { staged: 0, unstaged: 1, untracked: 0, conflicted: 0 },
      state: { kind: "merging" },
    });
    expect(freshOverview(opened, now)).toMatchObject({
      branch: "feature",
      ahead: 2,
      behind: 1,
      dirty: true,
      state: { kind: "merging" },
    });
  });

  it("says a detached HEAD has no branch and nothing to push", () => {
    const now = summary({ head: { kind: "detached", oid: "c".repeat(40) } });
    expect(freshOverview(overview({ branch: "main", ahead: 3 }), now)).toMatchObject({
      branch: null,
      ahead: 0,
      behind: 0,
    });
  });

  it("leaves another repository's row as the list read it", () => {
    const other = overview({ repo: 7 as never });
    expect(freshOverview(other, summary())).toBe(other);
    expect(freshOverview(other, null)).toBe(other);
  });
});

// Submodule rows had no dot and no arrows at all (item 10 of 25.09).
describe("the marks of a submodule node", () => {
  const shown = {
    repo: 9,
    root: "/a/vendor/lib",
    name: "lib",
    isBare: false,
    head: { kind: "branch", name: "dev", oid: "a".repeat(40) },
    branches: [
      {
        name: "dev",
        fullName: "refs/heads/dev",
        kind: "local",
        oid: "a".repeat(40),
        isHead: true,
        upstream: "origin/dev",
        ahead: 2,
        behind: 0,
      },
    ],
    tags: [],
    status: { staged: 0, unstaged: 0, untracked: 1, conflicted: 0 },
    state: { kind: "clean" },
    indexLock: null,
    tagGroupSeparator: "/",
  } as unknown as RepoSummary;

  it("reads one off screen from its pulse", () => {
    const sync = moduleSync({ shown: null, pulse: pulse({ dirty: true, ahead: 1 }), fetchFailed: false });
    expect(sync).toMatchObject({ dirty: true, ahead: 1, behind: 3 });
  });

  it("reads the one the panels show from what they keep fresh, not from a stale pulse", () => {
    const sync = moduleSync({ shown, pulse: pulse({ dirty: false }), fetchFailed: false });
    expect(sync).toMatchObject({ dirty: true, ahead: 2, behind: 0, branch: "dev" });
  });

  it("claims nothing before its first pulse", () => {
    expect(moduleSync({ shown: null, pulse: undefined, fetchFailed: false }).dirty).toBeNull();
  });

  it("keeps what the background check said of it", () => {
    const sync = moduleSync({ shown: null, pulse: pulse({ behind: 0 }), fetchFailed: false, remoteAhead: true });
    expect(canPull(sync)).toBe(true);
    expect(moduleSync({ shown: null, pulse: pulse(), fetchFailed: true }).unknown).toBe(true);
  });
});

describe("the marks the panels hand to a row they let go of", () => {
  it("are those of the summary they showed, upstream included", () => {
    const now = {
      repo: 3,
      root: "/a",
      name: "a",
      isBare: false,
      head: { kind: "branch", name: "dev", oid: "a".repeat(40) },
      branches: [
        { name: "dev", fullName: "refs/heads/dev", kind: "local", oid: "a".repeat(40), isHead: true, upstream: "origin/dev", ahead: 0, behind: 2 },
      ],
      tags: [],
      status: { staged: 1, unstaged: 0, untracked: 0, conflicted: 0 },
      state: { kind: "clean" },
      indexLock: null,
      tagGroupSeparator: "/",
    } as unknown as RepoSummary;
    expect(summaryPulse(now)).toEqual({ missing: false, branch: "dev", tracked: true, ahead: 0, behind: 2, dirty: true });
  });
});
