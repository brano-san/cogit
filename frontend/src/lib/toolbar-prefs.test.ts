import { describe, expect, it } from "vitest";
import {
  DEFAULT_PREFS,
  currentRemote,
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
});
