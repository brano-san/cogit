import { beforeEach, expect, it, vi } from "vitest";
import { WINDOW_LAYOUTS, workingTreeFiles } from "$lib/window-layout";
import { DEFAULT_LAYOUT } from "$lib/perspectives";
import { DEFAULT_SETTINGS, merge } from "$lib/settings";
import { LayoutStore } from "./layout.svelte";

const file = vi.hoisted(() => new Map<string, unknown>());
vi.mock("$lib/settings-file", () => ({
  readKey: async (key: string) => file.get(key),
  writeKey: async (key: string, value: unknown) => file.set(key, JSON.parse(JSON.stringify(value))),
}));
beforeEach(() => file.clear());

it("restores independent layout sizes, visibility and the active choice", async () => {
  let choice = DEFAULT_SETTINGS.activeLayout;
  const store = new LayoutStore(() => choice);
  await store.load();
  expect(store.fractions).toEqual(DEFAULT_LAYOUT);
  store.set("graph", 0.58);
  choice = "log";
  expect(store.visible("commit")).toBe(false);
  expect(store.visible("graph")).toBe(true);
  expect(store.visible("repositories")).toBe(true);
  store.set("graph", 0.77);
  choice = "workingTree";
  expect(store.visible("graph")).toBe(false);
  expect(store.visible("files")).toBe(true);
  expect(store.visible("commit")).toBe(true);
  store.set("topRow", 0.48);
  choice = "standard";
  expect(store.fractions.graph).toBe(0.58);
  await store.saved();
  const restored = new LayoutStore(() => choice);
  await restored.load();
  choice = "log";
  expect(restored.fractions.graph).toBe(0.77);
  restored.resetSizes();
  expect(restored.fractions.graph).not.toBe(0.77);
  choice = "workingTree";
  expect(restored.fractions.topRow).toBe(0.48);
  expect(merge({ ...DEFAULT_SETTINGS, activeLayout: choice }).activeLayout).toBe(choice);
  expect(merge({ activeLayout: "unknown" as typeof choice }).activeLayout).toBe("standard");
});

it("keeps legacy Standard perspective sizes during migration", async () => {
  const store = new LayoutStore(() => "standard", { main: { fractions: { graph: 0.73 }, hidden: ["worktrees"] } });
  await store.load();
  expect(store.fractions.graph).toBe(0.73);
  expect(store.visible("worktrees")).toBe(false);
  store.switch("review");
  expect(store.visible("commit")).toBe(false);
});

it("selects the files for the layout without changing the graph selection", () => {
  for (const oid of [null, "123"]) {
    expect(workingTreeFiles("workingTree", oid)).toBe(true);
    expect(workingTreeFiles("log", oid)).toBe(false);
    expect(workingTreeFiles("standard", oid)).toBe(oid === null);
  }
  expect(WINDOW_LAYOUTS.workingTree.diff).toBe("beside");
  expect(WINDOW_LAYOUTS.log.diff).toBe("below");
  expect(WINDOW_LAYOUTS.standard.diff).toBe("below");
});

it("retains panel state across layout switches and resets only splitter sizes", async () => {
  let id: typeof DEFAULT_SETTINGS.activeLayout = "standard";
  const store = new LayoutStore(() => id);
  await store.load();
  store.togglePanel("worktrees");
  store.toggleMaximized("diff");
  id = "log";
  expect(store.maximized).toBeNull();
  store.togglePanel("refs");
  store.set("topRow", 0.8);
  store.resetSizes();
  expect(store.visible("refs")).toBe(false);
  id = "standard";
  expect(store.maximized).toBe("diff");
  store.toggleMaximized("diff");
  expect(store.visible("worktrees")).toBe(false);
  await store.saved();
});
