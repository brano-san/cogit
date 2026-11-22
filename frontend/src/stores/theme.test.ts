import { afterEach, expect, it, vi } from "vitest";

vi.mock("$lib/ipc", () => ({ readUserTheme: async () => "{}" }));
vi.mock("$lib/ipc/file-menus", () => ({ setNativeTheme: async () => {} }));

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
