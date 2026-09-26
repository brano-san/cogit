import { beforeEach, describe, expect, it, vi } from "vitest";

const disk = vi.hoisted(() => ({ text: "{}", reads: 0 }));

vi.mock("$lib/ipc", () => ({
  readSettings: async () => {
    disk.reads += 1;
    return disk.text;
  },
  writeSetting: async () => null,
}));

const { forgetSettings, readKey } = await import("./settings-file");

beforeEach(() => {
  forgetSettings();
  disk.reads = 0;
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
});
