import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "./settings";
import { FIRST_RUN_STEPS, finishFirstRun, firstRunChoices } from "./first-run";

describe("the first-run wizard", () => {
  it("asks about the theme, avatars, updates and the perspective, in that order", () => {
    expect(FIRST_RUN_STEPS).toEqual(["theme", "avatars", "updates", "perspective"]);
  });

  it("starts from the current settings, an undecided avatar choice as Gravatar", () => {
    expect(firstRunChoices({ ...DEFAULT_SETTINGS, theme: "light", avatars: "ask" })).toEqual({
      theme: "light",
      avatars: "gravatar",
      autoUpdate: true,
      startupPerspective: "main",
    });
  });

  it("saves the choices and is not shown again", () => {
    const done = finishFirstRun(DEFAULT_SETTINGS, {
      theme: "graphite",
      avatars: "off",
      autoUpdate: false,
      startupPerspective: "review",
    });
    expect(done).toMatchObject({
      theme: "graphite",
      avatars: "off",
      autoUpdate: false,
      startupPerspective: "review",
      firstRunDone: true,
    });
  });
});
