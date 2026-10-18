import { beforeEach, describe, expect, it, vi } from "vitest";

const disk = vi.hoisted(() => ({ text: "{}", reads: 0, locked: false, written: [] as string[] }));

vi.mock("$lib/ipc", () => ({
  readSettings: async () => {
    disk.reads += 1;
    if (disk.locked) throw new Error("The process cannot access the file");
    return disk.text;
  },
  writeSetting: async (key: string) => {
    disk.written.push(key);
    return null;
  },
}));

const { forgetSettings, readKey, writeKey } = await import("./settings-file");

beforeEach(() => {
  forgetSettings();
  disk.reads = 0;
  disk.locked = false;
  disk.written = [];
});

// A Blame window kept its first reading of the file for as long as it was open: Preferences
// ▸ Theme in the main window never reached it, even had it asked again.
describe("settings-file", () => {
  it("reads the file once for any number of keys", async () => {
    disk.text = JSON.stringify({ settings: { theme: "dark" }, keymap: {} });
    await readKey("settings");
    await readKey("keymap");
    expect(disk.reads).toBe(1);
  });

  it("reads the file again once told another window wrote it", async () => {
    disk.text = JSON.stringify({ settings: { theme: "dark" } });
    expect(await readKey("settings")).toEqual({ theme: "dark" });
    disk.text = JSON.stringify({ settings: { theme: "light" } });
    forgetSettings();
    expect(await readKey("settings")).toEqual({ theme: "light" });
    expect(disk.reads).toBe(2);
  });

  // A scanner held the file at start: the page took defaults, and the next change of one
  // preference wrote them over every other one.
  it("does not write a key whose read failed", async () => {
    disk.locked = true;
    expect(await readKey("settings")).toBeUndefined();
    disk.locked = false;
    disk.text = JSON.stringify({ settings: { theme: "dark" } });
    await expect(writeKey("settings", { theme: "light" })).rejects.toThrow();
    expect(disk.written).toEqual([]);
  });

  it("writes it again once it was read", async () => {
    disk.locked = true;
    await readKey("settings");
    disk.locked = false;
    forgetSettings();
    await readKey("settings");
    await writeKey("settings", { theme: "light" });
    expect(disk.written).toEqual(["settings"]);
  });

  it("writes a key nobody read", async () => {
    disk.locked = true;
    await readKey("settings");
    disk.locked = false;
    await writeKey("toolbar", {});
    expect(disk.written).toEqual(["toolbar"]);
  });
});
