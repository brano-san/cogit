import { describe, expect, it } from "vitest";
import {
  DEFAULT_PREFS,
  currentRemote,
  mergePrefs,
  pullSteps,
  remotesInOrder,
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
