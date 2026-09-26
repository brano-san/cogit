import { beforeEach, describe, expect, it, vi } from "vitest";

const stored = vi.hoisted(() => new Map<string, unknown>());
const announced = vi.hoisted(() => [] as number[]);
const forgets = vi.hoisted(() => ({ count: 0 }));
const forgotten = () => forgets.count;

vi.mock("$lib/settings-file", () => ({
  readKey: async (key: string) => stored.get(key),
  writeKey: async (key: string, value: unknown) => void stored.set(key, value),
  forgetSettings: () => void (forgets.count += 1),
}));
vi.mock("$lib/settings-sync", () => ({ announceSettings: () => void announced.push(1) }));
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands: {}, events: {} }));

const { settings } = await import("./settings.svelte");

beforeEach(() => {
  stored.clear();
  stored.set("settings", { dateFormat: "iso", contextLines: 5 });
  stored.set("keymap", { fetch: "CmdOrCtrl+Alt+F" });
});

// The Compare and Blame windows read the settings inside an effect that also read them:
// every load put a new object in `current`, the effect ran again, loaded again, and the
// window spun on `diff_file` for as long as it was open.
describe("reading the settings again", () => {
  it("keeps the same objects when the file has not changed", async () => {
    await settings.load();
    const current = settings.current;
    const keymap = settings.keymap;

    await settings.load();

    expect(settings.current).toBe(current);
    expect(settings.keymap).toBe(keymap);
  });

  it("still takes a change made to the file", async () => {
    await settings.load();
    stored.set("settings", { dateFormat: "relative", contextLines: 5 });

    await settings.load();

    expect(settings.current.dateFormat).toBe("relative");
  });
});

// The child windows read the settings once, on opening; a theme chosen in the main window
// after that never reached them (F-335).
describe("following another window", () => {
  it("tells the other windows each time it writes the settings", async () => {
    announced.length = 0;
    await settings.set("dateFormat", "relative");
    await settings.apply({ ...settings.current, contextLines: 7 });
    await settings.setKeymap({});
    expect(announced).toHaveLength(3);
  });

  it("reads the file afresh and recolours the window", async () => {
    const root = { dataset: {} as Record<string, string> };
    vi.stubGlobal("document", { documentElement: root });
    await settings.load();
    stored.set("settings", { theme: "light" });

    await settings.reload();

    expect(forgotten()).toBeGreaterThan(0);
    expect(settings.current.theme).toBe("light");
    expect(root.dataset.theme).toBe("light");
    vi.unstubAllGlobals();
  });
});
