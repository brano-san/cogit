import { describe, expect, it } from "vitest";
import type { PushChoice } from "./network-dialogs";
import { effectiveChoice, upstreamState } from "./push-dialog";

const choice = (over: Partial<PushChoice> = {}): PushChoice => ({
  remote: "origin",
  local: "main",
  branch: "main",
  setUpstream: true,
  tags: "none",
  notes: false,
  forceWithLease: false,
  ...over,
});

describe("Set as upstream", () => {
  it("says the branch already tracks the target and sends no --set-upstream", () => {
    expect(upstreamState("origin/main", "origin", "main")).toEqual({
      kind: "already",
      hint: "Already tracking origin/main",
    });
    expect(effectiveChoice(choice(), "origin/main").setUpstream).toBe(false);
  });

  it("names the upstream it replaces", () => {
    expect(upstreamState("origin/old", "origin", "main").hint).toBe("Track origin/main instead of origin/old");
    expect(effectiveChoice(choice(), "origin/old").setUpstream).toBe(true);
  });

  it("says when there is no upstream yet, slashes and all", () => {
    expect(upstreamState(null, "fork", "feature/x").hint).toBe("Track fork/feature/x (no upstream yet)");
    expect(upstreamState("fork/feature/x", "fork", "feature/x").kind).toBe("already");
  });
});
