import { describe, expect, it } from "vitest";
import { canPull, REMOTE_AHEAD, rowSync, syncTooltip, UNKNOWN_PULL } from "./repo-sync";
import type { RepoOverview } from "$lib/ipc";
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
