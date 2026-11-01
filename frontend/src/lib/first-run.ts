import type { Settings } from "./settings";

export const FIRST_RUN_STEPS = ["theme", "avatars", "updates", "perspective"] as const;
export type FirstRunStep = (typeof FIRST_RUN_STEPS)[number];

export type FirstRunChoices = Pick<Settings, "theme" | "autoUpdate" | "startupPerspective"> & {
  avatars: "gravatar" | "off";
};

export function firstRunChoices(settings: Settings): FirstRunChoices {
  return {
    theme: settings.theme,
    avatars: settings.avatars === "off" ? "off" : "gravatar",
    autoUpdate: settings.autoUpdate,
    startupPerspective: settings.startupPerspective,
  };
}

export function finishFirstRun(settings: Settings, choices: FirstRunChoices): Settings {
  return { ...settings, ...choices, firstRunDone: true };
}
