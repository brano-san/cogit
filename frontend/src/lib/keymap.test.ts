import { describe, expect, it } from "vitest";
import type { KeyBinding } from "$lib/ipc/bindings";
import cases from "./accelerator-cases.json";
import {
  claimable,
  conflicts,
  effective,
  mergeKeymap,
  prettyKeys,
  recordKeys,
  type KeyPress,
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

describe("recordKeys", () => {
  const press = (over: Partial<KeyPress>): KeyPress => ({
    key: "a",
    code: "KeyA",
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    ...over,
  });
  const keysOf = (over: Partial<KeyPress>, onMac = false) => {
    const recorded = recordKeys(press(over), onMac);
    return recorded && "keys" in recorded ? recorded.keys : recorded;
  };

  it("writes Ctrl as CmdOrCtrl so one binding works on every platform", () => {
    expect(keysOf({ key: "p", code: "KeyP", ctrlKey: true })).toBe("CmdOrCtrl+P");
    expect(keysOf({ key: "p", code: "KeyP", metaKey: true }, true)).toBe("CmdOrCtrl+P");
  });

  it("keeps the modifiers in a fixed order", () => {
    expect(keysOf({ key: "p", code: "KeyP", ctrlKey: true, shiftKey: true, altKey: true })).toBe(
      "CmdOrCtrl+Alt+Shift+P",
    );
  });

  it("takes a function key alone or with modifiers", () => {
    expect(keysOf({ key: "F5", code: "F5" })).toBe("F5");
    expect(keysOf({ key: "F11", code: "F11", shiftKey: true })).toBe("Shift+F11");
  });

  // muda reads `Enter` and not `Return`: a recorded Ctrl+Return was dropped from the menu.
  it("names Enter and the arrows as the menu reads them", () => {
    expect(keysOf({ key: "Enter", code: "Enter", ctrlKey: true })).toBe("CmdOrCtrl+Enter");
    expect(keysOf({ key: "Enter", code: "NumpadEnter", ctrlKey: true })).toBe("CmdOrCtrl+Enter");
    expect(keysOf({ key: "ArrowLeft", code: "ArrowLeft", altKey: true })).toBe("Alt+Left");
  });

  // The recorder took the character the layout typed: on a Russian keyboard Ctrl+S was
  // written down as CmdOrCtrl+Ы, which no menu can read, so the key silently did nothing.
  it("records the key where it is, whatever the keyboard layout types there", () => {
    expect(keysOf({ key: "ы", code: "KeyS", ctrlKey: true })).toBe("CmdOrCtrl+S");
    expect(keysOf({ key: "&", code: "Digit7", ctrlKey: true, shiftKey: true })).toBe("CmdOrCtrl+Shift+7");
    expect(keysOf({ key: "б", code: "Comma", ctrlKey: true })).toBe("CmdOrCtrl+,");
    expect(keysOf({ key: "+", code: "Equal", ctrlKey: true, shiftKey: true })).toBe("CmdOrCtrl+Shift+=");
  });

  it("waits through a bare modifier, which is not a shortcut", () => {
    expect(recordKeys(press({ key: "Control", code: "ControlLeft", ctrlKey: true }), false)).toBeNull();
    expect(recordKeys(press({ key: "Shift", code: "ShiftLeft", shiftKey: true }), false)).toBeNull();
  });

  // Ctrl+Up, Ctrl+/ or a lone letter showed as assigned, and pressing it did nothing: the
  // window never claims it, and WebView2 keeps it from the menu.
  it("refuses what the window would never run, and says which key it was", () => {
    const refused = (over: Partial<KeyPress>) => {
      const recorded = recordKeys(press(over), false);
      return recorded !== null && "refused" in recorded ? recorded.refused : null;
    };
    expect(refused({ key: "p", code: "KeyP" })).toBe("P cannot be used here");
    expect(refused({ key: "P", code: "KeyP", shiftKey: true })).toBe("Shift+P cannot be used here");
    expect(refused({ key: "ArrowUp", code: "ArrowUp" })).toBe("Up cannot be used here");
    expect(refused({ key: " ", code: "Space", ctrlKey: true })).toBe("Ctrl+Space cannot be used here");
    expect(refused({ key: "/", code: "Slash", ctrlKey: true })).toBe("Ctrl+/ cannot be used here");
    expect(refused({ key: "Tab", code: "Tab", ctrlKey: true })).toBe("Ctrl+Tab cannot be used here");
    expect(refused({ key: "Delete", code: "Delete", ctrlKey: true })).toBe("Ctrl+Delete cannot be used here");
  });

  // AltGr arrives as Ctrl and Alt and types a character; the window lets it through (R-516).
  it("refuses a key pressed with AltGr", () => {
    const altGr = press({ key: "ś", code: "KeyS", ctrlKey: true, altKey: true, getModifierState: (key) => key === "AltGraph" });
    expect(recordKeys(altGr, false)).toEqual({ refused: "AltGr+S cannot be used here" });
  });

  it("refuses the other platform's modifier, which CmdOrCtrl does not mean", () => {
    expect(recordKeys(press({ key: "s", code: "KeyS", metaKey: true }), false)).toEqual({
      refused: "Win+S cannot be used here",
    });
    expect(recordKeys(press({ key: "s", code: "KeyS", ctrlKey: true }), true)).toEqual({
      refused: "Ctrl+S cannot be used here",
    });
  });

  it("keeps the keys text fields and panels live on, and the system's own", () => {
    for (const code of ["KeyA", "KeyC", "KeyV", "KeyX", "KeyZ", "KeyY", "KeyF"]) {
      const recorded = recordKeys(press({ key: code.slice(3).toLowerCase(), code, ctrlKey: true }), false);
      expect(recorded).toEqual({ refused: `Ctrl+${code.slice(3)} is kept for text fields and panels` });
    }
    expect(recordKeys(press({ key: "Z", code: "KeyZ", ctrlKey: true, shiftKey: true }), false)).toEqual({
      refused: "Ctrl+Shift+Z is kept for text fields and panels",
    });
    expect(recordKeys(press({ key: "F4", code: "F4", altKey: true }), false)).toEqual({
      refused: "Alt+F4 is the system's",
    });
  });
});

describe("claimable", () => {
  it("agrees with the window's dispatcher on every shared case", () => {
    for (const keys of cases.claimable) expect(claimable(keys), keys).toBe(true);
    for (const keys of cases.refused) expect(claimable(keys), keys).toBe(false);
  });

  it("holds every key it records", () => {
    const recorded = recordKeys(
      { key: "б", code: "Comma", ctrlKey: true, shiftKey: false, altKey: true, metaKey: false },
      false,
    );
    expect(recorded).toEqual({ keys: "CmdOrCtrl+Alt+," });
    expect(claimable("CmdOrCtrl+Alt+,")).toBe(true);
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
