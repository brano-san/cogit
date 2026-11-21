import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "./settings";
import { FIRST_RUN_STEPS, finishFirstRun, firstRunChoices } from "./first-run";

describe("the first-run wizard", () => {
  it("asks about layout after the existing first-run choices", () => {
    expect(FIRST_RUN_STEPS).toEqual(["theme", "avatars", "updates", "perspective", "layout"]);
  });

  it("starts from the current settings, an undecided avatar choice as Gravatar", () => {
    expect(firstRunChoices({ ...DEFAULT_SETTINGS, theme: "light", avatars: "ask" })).toEqual({
      theme: "light",
      avatars: "gravatar",
      autoUpdate: true,
      startupPerspective: "main",
      activeLayout: "standard",
    });
  });

  it.each(["standard", "log", "workingTree"] as const)("saves the %s layout on Finish", (activeLayout) => {
    const done = finishFirstRun(DEFAULT_SETTINGS, {
      theme: "graphite",
      avatars: "off",
      autoUpdate: false,
      startupPerspective: "review",
      activeLayout,
    });
    expect(done).toMatchObject({
      theme: "graphite",
      avatars: "off",
      autoUpdate: false,
      startupPerspective: "review",
      activeLayout,
      firstRunDone: true,
    });
  });

  it("keeps the original layout on Skip", () => {
    const before = firstRunChoices(DEFAULT_SETTINGS);
    const draft = { ...before, activeLayout: "workingTree" as const };
    expect(finishFirstRun(DEFAULT_SETTINGS, before).activeLayout).toBe("standard");
    expect(finishFirstRun(DEFAULT_SETTINGS, draft).activeLayout).toBe("workingTree");
  });
});
