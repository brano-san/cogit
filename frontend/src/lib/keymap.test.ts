import { describe, expect, it } from "vitest";
import type { KeyBinding } from "$lib/ipc/bindings";
import {
  accelerator,
  conflicts,
  effective,
  mergeKeymap,
  prettyKeys,
  type Keymap,
} from "./keymap";

const binding = (id: string, keys: string | null, section = "Query"): KeyBinding => ({
  id,
  label: id,
  section,
  defaultAccelerator: keys,
});

const DEFAULTS = [
  binding("find", "CmdOrCtrl+P"),
  binding("palette", "CmdOrCtrl+Shift+P"),
  binding("blame", null),
];

describe("effective", () => {
  it("uses the shipped key when there is no override", () => {
    expect(effective(DEFAULTS, {})["find"]).toBe("CmdOrCtrl+P");
  });

  it("prefers the user's key", () => {
    expect(effective(DEFAULTS, { find: "F3" })["find"]).toBe("F3");
  });

  it("treats an empty override as no key at all", () => {
    expect(effective(DEFAULTS, { find: "" })["find"]).toBe("");
  });

  it("leaves a command that never had a key without one", () => {
    expect(effective(DEFAULTS, {})["blame"]).toBe("");
  });

  it("ignores an override for a command that does not exist", () => {
    expect(effective(DEFAULTS, { nonsense: "F9" })["nonsense"]).toBeUndefined();
  });
});

describe("conflicts", () => {
  it("finds nothing when every key is distinct", () => {
    expect(conflicts(DEFAULTS, {})).toEqual({});
  });

  it("names both commands that claim one key", () => {
    const clash = conflicts(DEFAULTS, { blame: "CmdOrCtrl+P" });
    expect(clash["blame"]).toEqual(["find"]);
    expect(clash["find"]).toEqual(["blame"]);
  });

  it("does not call an unbound command a conflict", () => {
    expect(conflicts(DEFAULTS, { find: "" })).toEqual({});
  });

  it("reports all three when three commands share a key", () => {
    const all = [...DEFAULTS, binding("about", null, "Help")];
    const clash = conflicts(all, { blame: "F5", about: "F5", find: "F5" });
    expect(clash["find"]?.sort()).toEqual(["about", "blame"]);
  });
});

describe("accelerator", () => {
  const event = (over: Partial<KeyboardEvent>) =>
    ({ key: "a", ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...over }) as KeyboardEvent;

  it("reads a plain letter as its uppercase self", () => {
    expect(accelerator(event({ key: "p" }))).toBe("P");
  });

  it("writes Ctrl as CmdOrCtrl so one binding works on every platform", () => {
    expect(accelerator(event({ key: "p", ctrlKey: true }))).toBe("CmdOrCtrl+P");
  });

  it("keeps the modifiers in a fixed order", () => {
    const keys = accelerator(event({ key: "p", ctrlKey: true, shiftKey: true, altKey: true }));
    expect(keys).toBe("CmdOrCtrl+Alt+Shift+P");
  });

  it("names a function key as it is", () => {
    expect(accelerator(event({ key: "F5" }))).toBe("F5");
  });

  it("names the named keys tauri expects", () => {
    expect(accelerator(event({ key: "Enter" }))).toBe("Return");
    expect(accelerator(event({ key: "ArrowLeft" }))).toBe("Left");
    expect(accelerator(event({ key: " " }))).toBe("Space");
  });

  it("refuses a bare modifier, which is not a shortcut", () => {
    expect(accelerator(event({ key: "Control", ctrlKey: true }))).toBeNull();
    expect(accelerator(event({ key: "Shift", shiftKey: true }))).toBeNull();
  });
});

describe("prettyKeys", () => {
  it("writes the platform's own word for the modifier", () => {
    expect(prettyKeys("CmdOrCtrl+Shift+P", false)).toBe("Ctrl+Shift+P");
    expect(prettyKeys("CmdOrCtrl+Shift+P", true)).toBe("Cmd+Shift+P");
  });

  it("says so when there is no key", () => {
    expect(prettyKeys("", false)).toBe("—");
  });
});

describe("mergeKeymap", () => {
  it("keeps only string values", () => {
    expect(mergeKeymap({ find: "F3", bad: 7 } as unknown)).toEqual({ find: "F3" });
  });

  it("survives a missing store", () => {
    expect(mergeKeymap(null)).toEqual({} as Keymap);
  });
});
