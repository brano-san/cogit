import { describe, expect, it } from "vitest";
import {
  DEFAULT_PREFS,
  currentRemote,
  headRemote,
  mergePrefs,
  pullSteps,
  remotePlan,
  remotesInOrder,
  syncSteps,
} from "./toolbar-prefs";

describe("mergePrefs", () => {
  it("starts from the defaults: current remote, nothing deleted", () => {
    expect(mergePrefs(undefined)).toEqual(DEFAULT_PREFS);
    expect(DEFAULT_PREFS.pullScope).toBe("current");
    expect(DEFAULT_PREFS.deleteMergedAfterPull).toBe(false);
  });

  it("keeps what was stored", () => {
    expect(mergePrefs({ pullScope: "all", deleteMergedAfterPull: true })).toMatchObject({
      pullScope: "all",
      deleteMergedAfterPull: true,
    });
  });

  it("drops a bad value and keeps the good ones", () => {
    expect(mergePrefs({ pullScope: "everything", deleteMergedAfterPull: true })).toMatchObject({
      pullScope: "current",
      deleteMergedAfterPull: true,
    });
  });
});

describe("currentRemote", () => {
  it("is the remote the branch tracks", () => {
    expect(currentRemote("upstream/main", ["origin", "upstream"])).toBe("upstream");
  });

  it("prefers the longest name when one remote name prefixes another", () => {
    expect(currentRemote("team/tools/main", ["team", "team/tools"])).toBe("team/tools");
  });

  it("falls back to origin, then to the first remote", () => {
    expect(currentRemote(null, ["custom", "origin"])).toBe("origin");
    expect(currentRemote(undefined, ["custom", "other"])).toBe("custom");
  });

  it("is nothing without remotes", () => {
    expect(currentRemote("origin/main", [])).toBeNull();
  });
});

describe("remotesInOrder", () => {
  it("puts the current remote first and the rest in alphabetical order", () => {
    expect(remotesInOrder(["origin", "custom-controls", "beta"], "origin")).toEqual([
      "origin",
      "beta",
      "custom-controls",
    ]);
  });
});

describe("pullSteps", () => {
  it("pulls from the current remote alone by default", () => {
    expect(pullSteps("current", ["origin", "fork"], "origin")).toEqual({ fetch: [], pull: "origin" });
  });

  it("fetches every other remote first when set to all", () => {
    expect(pullSteps("all", ["origin", "fork", "beta"], "origin")).toEqual({
      fetch: ["beta", "fork"],
      pull: "origin",
    });
  });
});

describe("syncSteps", () => {
  it("pulls, then pushes by default", () => {
    expect(DEFAULT_PREFS.syncOrder).toBe("pullThenPush");
    expect(syncSteps(DEFAULT_PREFS.syncOrder)).toEqual(["pull", "push"]);
  });

  it("pushes first when told to", () => {
    expect(syncSteps("pushThenPull")).toEqual(["push", "pull"]);
  });

  it("remembers the order and ignores a bad one", () => {
    expect(mergePrefs({ syncOrder: "pushThenPull" }).syncOrder).toBe("pushThenPull");
    expect(mergePrefs({ syncOrder: "sideways" }).syncOrder).toBe("pullThenPush");
  });
});

// Sync ran its steps against whatever the panels showed when each began: move to B during
// the pull, and A was pushed to the remote B's list named, or not at all.
describe("remotePlan", () => {
  const facts = {
    remotes: ["origin", "fork"],
    pullRemote: "origin",
    pushRemote: "origin",
    scope: "all" as const,
    ffOnly: true,
    deleteMerged: true,
  };

  it("names every remote of a Sync before the first step runs", () => {
    expect(remotePlan(["pull", "push"], facts)).toEqual([
      { kind: "fetch", remote: "fork" },
      { kind: "pull", remote: "origin", ffOnly: true },
      { kind: "deleteMerged" },
      { kind: "push", remote: "origin" },
    ]);
  });

  it("keeps the order chosen for Sync", () => {
    const plan = remotePlan(["push", "pull"], { ...facts, scope: "current", deleteMerged: false });
    expect(plan.map((step) => step.kind)).toEqual(["push", "pull"]);
  });

  it("refuses up front when there is no remote", () => {
    expect(() => remotePlan(["pull"], { ...facts, remotes: [], pullRemote: null, pushRemote: null })).toThrow(
      "This repository has no remote.",
    );
  });

  // F-320: one commit ahead and one behind, Pull set to fast-forward only (the default).
  // `git pull --ff-only` failed with "Not possible to fast-forward" and Synchronize stopped
  // there, saying nothing about what to do.
  const diverged = { name: "main", upstream: "origin/main", ahead: 1, behind: 1 };

  it("refuses a diverged branch before anything runs when Pull only fast-forwards, and says why", () => {
    expect(() => remotePlan(["pull", "push"], { ...facts, branch: diverged })).toThrow(
      /main and origin\/main have diverged \(1 ahead, 1 behind\).*Preferences ▸ Pull/,
    );
  });

  it("merges a diverged branch, then pushes, when Pull is set to merge", () => {
    const plan = remotePlan(["pull", "push"], {
      ...facts,
      scope: "current",
      deleteMerged: false,
      ffOnly: false,
      branch: diverged,
    });
    expect(plan).toEqual([
      { kind: "pull", remote: "origin", ffOnly: false },
      { kind: "push", remote: "origin" },
    ]);
  });

  it("pulls a branch that is only behind, however Pull is set", () => {
    const behind = { ...diverged, ahead: 0, behind: 2 };
    expect(remotePlan(["pull"], { ...facts, scope: "current", deleteMerged: false, branch: behind })).toEqual([
      { kind: "pull", remote: "origin", ffOnly: true },
    ]);
  });
});

// Pull from a row's menu went to origin while main tracked upstream/main: git refused
// with "You asked to pull from the remote 'origin', but did not specify a branch".
describe("headRemote", () => {
  const branch = (name: string, upstream: string | null) =>
    ({ name, fullName: `refs/heads/${name}`, kind: "local", oid: "c1", isHead: false, upstream, ahead: 0, behind: 0 }) as const;

  it("is the remote HEAD's branch tracks", () => {
    const refs = { head: { kind: "branch", name: "main", oid: "c1" } as const, branches: [branch("main", "upstream/main")] };
    expect(headRemote(refs, ["origin", "upstream"])).toBe("upstream");
  });

  it("falls back like currentRemote on a detached HEAD or an untracked branch", () => {
    expect(headRemote({ head: { kind: "detached", oid: "c1" }, branches: [] }, ["origin", "upstream"])).toBe("origin");
    const refs = { head: { kind: "branch", name: "topic", oid: "c1" } as const, branches: [branch("topic", null)] };
    expect(headRemote(refs, ["fork"])).toBe("fork");
  });
});
