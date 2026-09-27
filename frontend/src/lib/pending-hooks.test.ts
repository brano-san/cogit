import { describe, expect, it } from "vitest";
import type { HookOverview } from "$lib/ipc";
import { hooksNote, pendingHooks } from "./pending-hooks";

const hook = (name: string, state: "enabled" | "disabled" | "missing") => ({
  name,
  description: "",
  state,
  source: null,
  executable: true,
  size: 1,
});

const overview = {
  activeDir: ".git/hooks",
  source: "gitHooks",
  configuredPath: null,
  availablePath: null,
  hooks: [hook("commit-msg", "enabled"), hook("pre-commit", "enabled"), hook("pre-push", "disabled")],
} as HookOverview;

describe("pendingHooks", () => {
  it("lists the enabled hooks of a step in the order git runs them", () => {
    expect(pendingHooks(overview, "commit")).toEqual(["pre-commit", "commit-msg"]);
    expect(pendingHooks(overview, "push")).toEqual([]);
    expect(pendingHooks(null, "commit")).toEqual([]);
  });

  it("says nothing when no hook runs", () => {
    expect(hooksNote([])).toBeUndefined();
    expect(hooksNote(["pre-push"])).toContain("pre-push");
  });
});
