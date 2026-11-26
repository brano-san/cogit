import { afterEach, expect, it, vi } from "vitest";

vi.mock("$lib/ipc", () => ({ readUserTheme: async () => "{}" }));
vi.mock("$lib/ipc/file-menus", () => ({ setNativeTheme: async () => {} }));

import { THEME_FILES } from "$lib/theme";
import { bootTheme, themeStore } from "./theme.svelte";

afterEach(() => {
  themeStore.apply("dark");
  vi.unstubAllGlobals();
});

it("resolves the cached System choice live and stops following after a fixed choice", () => {
  const media = Object.assign(new EventTarget(), { matches: false });
  const root = { dataset: {} as Record<string, string>, style: { setProperty: vi.fn() } };
  const storage = new Map([["cogit.theme", "system"]]);
  vi.stubGlobal("document", { documentElement: root });
  vi.stubGlobal("localStorage", { getItem: storage.get.bind(storage), setItem: storage.set.bind(storage) });
  vi.stubGlobal("matchMedia", (query: string) => {
    expect(query).toBe("(prefers-color-scheme: dark)");
    return media;
  });

  bootTheme();
  expect(root.dataset.theme).toBe("light");
  expect(storage.get("cogit.theme")).toBe("system");

  media.matches = true;
  media.dispatchEvent(new Event("change"));
  expect(root.dataset.theme).toBe("dark");

  themeStore.apply("lightGrey");
  media.matches = false;
  media.dispatchEvent(new Event("change"));
  expect(root.dataset.theme).toBe("lightGrey");
});

it("uses the active fixed theme's saved accent and restores defaults when its override is removed", () => {
  const media = Object.assign(new EventTarget(), { matches: false });
  const vars = new Map<string, string>();
  const root = { dataset: {}, style: { setProperty: vars.set.bind(vars) } };
  vi.stubGlobal("document", { documentElement: root });
  vi.stubGlobal("matchMedia", () => media);
  const accents = { light: "#74432e", dark: "#abbe78" };

  themeStore.apply("system", accents);
  expect(themeStore.resolved).toBe("light");
  expect(vars.get("--accent")).toBe(accents.light);

  media.matches = true;
  media.dispatchEvent(new Event("change"));
  expect(themeStore.resolved).toBe("dark");
  expect(vars.get("--accent")).toBe(accents.dark);

  themeStore.apply("lightGrey", accents);
  expect(vars.get("--accent")).toBe(THEME_FILES.lightGrey.tokens.accent);
  themeStore.apply("light", { dark: accents.dark });
  expect(vars.get("--accent")).toBe(THEME_FILES.light.tokens.accent);
});
