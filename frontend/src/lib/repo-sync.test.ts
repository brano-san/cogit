import { describe, expect, it } from "vitest";
import { canPull, freshOverview, REMOTE_AHEAD, rowSync, syncTooltip, UNKNOWN_PULL } from "./repo-sync";
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
    });
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
