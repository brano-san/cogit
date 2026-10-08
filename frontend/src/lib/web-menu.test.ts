import { describe, expect, it } from "vitest";
import type { MenuNode } from "$lib/ipc/bindings";
import { SEPARATOR, item, submenu } from "./context-menu";
import { barRows, contextRows, itemFor, opensBar } from "./web-menu";

describe("contextRows", () => {
  it("tidies separators at every depth and formats keys", () => {
    const rows = contextRows(
      [
        SEPARATOR,
        item("copy", "Copy", true, "CmdOrCtrl+C"),
        SEPARATOR,
        SEPARATOR,
        submenu("move", "Move To", [SEPARATOR, item("a", "A")]),
        SEPARATOR,
      ],
      false,
    );
    expect(rows.map((row) => (row.separator ? "-" : row.id))).toEqual(["copy", "-", "move"]);
    expect(rows[0]!.shortcut).toBe("Ctrl+C");
    expect(rows[2]!.children.map((row) => row.id)).toEqual(["a"]);
  });

  it("uses Cmd on a Mac and keeps a disabled row in", () => {
    const rows = contextRows([item("x", "X", false, "CmdOrCtrl+K")], true);
    expect(rows[0]).toMatchObject({ enabled: false, shortcut: "Cmd+K", checked: null });
  });

  it("has no shortcut for a row without keys", () => {
    expect(contextRows([item("x", "X")], false)[0]!.shortcut).toBeNull();
  });
});

const node = (id: string, extra: Partial<MenuNode> = {}): MenuNode => ({
  id,
  label: id,
  separator: false,
  accelerator: null,
  enabled: true,
  checked: null,
  children: [],
  ...extra,
});

describe("barRows", () => {
  const menus = [
    node("menu:view", {
      children: [node("output", { checked: false, accelerator: "CmdOrCtrl+Shift+7" }), node("push", { enabled: false })],
    }),
    node("menu:window", { children: [node("window-maximize", { label: "Maximize" })] }),
  ];

  it("starts from the model flags", () => {
    const [view] = barRows(menus, false, null);
    expect(view!.children.map((row) => [row.enabled, row.checked])).toEqual([
      [true, false],
      [false, null],
    ]);
    expect(view!.children[0]!.shortcut).toBe("Ctrl+Shift+7");
  });

  it("takes live state over the model once the application pushed it", () => {
    const state = { disabled: new Set(["output"]), checked: new Set(["output"]), maximized: false };
    const [view] = barRows(menus, false, state);
    expect(view!.children[0]).toMatchObject({ enabled: false, checked: true });
    expect(view!.children[1]!.enabled).toBe(true);
  });

  it("never switches a menu title off", () => {
    const state = { disabled: new Set(["menu:view"]), checked: new Set<string>(), maximized: false };
    expect(barRows(menus, false, state)[0]!.enabled).toBe(true);
  });

  it("calls Maximize Restore while the window is maximized", () => {
    const state = { disabled: new Set<string>(), checked: new Set<string>(), maximized: true };
    expect(barRows(menus, false, state)[1]!.children[0]!.label).toBe("Restore");
  });
});

describe("opensBar", () => {
  const press = (key: string, extra: Partial<Parameters<typeof opensBar>[0]> = {}) =>
    opensBar({ key, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...extra });

  it("answers F10 and Alt with a letter", () => {
    expect(press("F10")).toBe(true);
    expect(press("e", { altKey: true })).toBe(true);
  });

  it("leaves Ctrl+Alt chords and plain letters alone", () => {
    expect(press("e", { altKey: true, ctrlKey: true })).toBe(false);
    expect(press("e")).toBe(false);
    expect(press("F10", { ctrlKey: true })).toBe(false);
    expect(press("ArrowLeft", { altKey: true })).toBe(false);
  });
});

describe("itemFor", () => {
  const node = (id: string, accelerator: string | null, children: MenuNode[] = []): MenuNode => ({
    id,
    label: id,
    separator: false,
    accelerator,
    enabled: true,
    checked: null,
    children,
  });

  it("finds the item bound to a key at any depth", () => {
    const model = [node("file", null, [node("open", "CmdOrCtrl+O"), node("sub", null, [node("deep", "CmdOrCtrl+K")])])];
    expect(itemFor(model, "CmdOrCtrl+K")?.id).toBe("deep");
    expect(itemFor(model, "CmdOrCtrl+J")).toBeNull();
  });
});
