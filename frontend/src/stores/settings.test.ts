import { beforeEach, describe, expect, it, vi } from "vitest";

const stored = vi.hoisted(() => new Map<string, unknown>());

vi.mock("$lib/settings-file", () => ({
  readKey: async (key: string) => stored.get(key),
  writeKey: async (key: string, value: unknown) => void stored.set(key, value),
}));
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
