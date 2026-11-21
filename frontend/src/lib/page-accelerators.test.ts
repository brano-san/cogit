import { describe, expect, it } from "vitest";
import type { KeyBinding } from "$lib/ipc/bindings";
import { effective } from "./keymap";
import { pageCommand } from "./page-accelerators";

const binding = (id: string, keys: string, onBar: boolean): KeyBinding => ({
  id,
  label: id,
  section: "Local",
  defaultAccelerator: keys,
  onBar,
});

const bindings = [
  binding("commit-window", "CmdOrCtrl+K", false),
  binding("refresh", "F5", false),
  binding("commit-amend", "CmdOrCtrl+Shift+Enter", false),
  binding("stage", "CmdOrCtrl+T", true),
  binding("unstage", "CmdOrCtrl+Shift+T", true),
];
const keys = effective(bindings, {});
const linux = { windows: false, mac: false };

const press = (key: string, code: string, over: Partial<KeyboardEvent> = {}) =>
  ({ key, code, ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, ...over }) as KeyboardEvent;

// Ctrl+K lived only in the WebView2 hook: under Linux no menu item had it and nothing ran.
describe("pageCommand", () => {
  it("cycles tabs with the alternate keys and leaves unmodified paging and tab focus alone", () => {
    const tabs = [
      binding("tab-next", "Ctrl+Tab", true),
      binding("tab-previous", "Ctrl+Shift+Tab", true),
      binding("tab-next-alternate", "Ctrl+PageDown", false),
      binding("tab-previous-alternate", "Ctrl+PageUp", false),
    ];
    for (const mac of [true, false]) {
      const platform = { windows: false, mac };
      const keys = effective(tabs, {}, mac);
      for (const [key, id] of [["PageDown", "tab-next-alternate"], ["PageUp", "tab-previous-alternate"]] as const) {
        expect(pageCommand(press(key, key), tabs, keys, platform)).toBe(id);
        expect(pageCommand(press(key, key, { ctrlKey: false }), tabs, keys, platform)).toBeNull();
      }
      expect(pageCommand(press("Tab", "Tab"), tabs, keys, platform)).toBeNull();
      expect(pageCommand(press("PageDown", "PageDown", { ctrlKey: false, metaKey: true }), tabs, keys, platform)).toBeNull();
    }
  });

  it("runs a command that is not on the menu bar", () => {
    expect(pageCommand(press("k", "KeyK"), bindings, keys, linux)).toBe("commit-window");
    expect(pageCommand(press("F5", "F5", { ctrlKey: false }), bindings, keys, linux)).toBe("refresh");
    expect(pageCommand(press("Enter", "Enter", { shiftKey: true }), bindings, keys, linux)).toBe("commit-amend");
  });

  it("leaves a bar key to the native menu, so it does not run twice", () => {
    expect(pageCommand(press("t", "KeyT"), bindings, keys, linux)).toBeNull();
    expect(pageCommand(press("T", "KeyT", { shiftKey: true }), bindings, keys, linux)).toBeNull();
  });

  it("takes a bar key the menu missed under a non-Latin layout", () => {
    expect(pageCommand(press("е", "KeyT"), bindings, keys, linux)).toBe("stage");
    expect(pageCommand(press("Е", "KeyT", { shiftKey: true }), bindings, keys, linux)).toBe("unstage");
  });

  it("follows the user's keymap", () => {
    const moved = effective(bindings, { "commit-window": "CmdOrCtrl+Shift+K" });
    expect(pageCommand(press("k", "KeyK"), bindings, moved, linux)).toBeNull();
    expect(pageCommand(press("K", "KeyK", { shiftKey: true }), bindings, moved, linux)).toBe("commit-window");
    const removed = effective(bindings, { "commit-window": "" });
    expect(pageCommand(press("k", "KeyK"), bindings, removed, linux)).toBeNull();
  });

  it("requires Command on a Mac and matches only the modifiers held", () => {
    const mac = { windows: false, mac: true };
    expect(pageCommand(press("л", "KeyK", { ctrlKey: false, metaKey: true }), bindings, keys, mac)).toBe("commit-window");
    expect(pageCommand(press("k", "KeyK"), bindings, keys, mac)).toBeNull();
    expect(pageCommand(press("k", "KeyK", { metaKey: true }), bindings, keys, mac)).toBeNull();
    expect(pageCommand(press("F5", "F5"), bindings, keys, mac)).toBeNull();
    expect(pageCommand(press("F5", "F5", { ctrlKey: false, metaKey: true }), bindings, keys, linux)).toBeNull();
  });

  it("leaves everything to the window on Windows", () => {
    expect(pageCommand(press("k", "KeyK"), bindings, keys, { windows: true, mac: false })).toBeNull();
  });

  it("ignores a key no command has", () => {
    expect(pageCommand(press("q", "KeyQ"), bindings, keys, linux)).toBeNull();
  });
});
