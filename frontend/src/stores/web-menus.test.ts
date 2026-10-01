import { describe, expect, it } from "vitest";
import { SEPARATOR, item } from "$lib/context-menu";
import { webMenus } from "./web-menus.svelte";

describe("webMenus", () => {
  it("leaves menus to the platform until the host says otherwise", () => {
    expect(webMenus.chrome).toEqual({ webMenus: false, customTitlebar: false });
  });

  it("opens a context menu at once from the items it was given, tidied", () => {
    webMenus.openContext([SEPARATOR, item("a", "A", true, "CmdOrCtrl+A"), SEPARATOR], 10, 20);
    expect(webMenus.context?.rows.map((row) => row.id)).toEqual(["a"]);
    expect(webMenus.context).toMatchObject({ x: 10, y: 20 });
    webMenus.closeContext();
    expect(webMenus.context).toBeNull();
  });

  it("opens nothing for a menu with nothing in it", () => {
    webMenus.openContext([SEPARATOR], 0, 0);
    expect(webMenus.context).toBeNull();
  });

  it("shows the state the application pushed in the bar", () => {
    webMenus.model = [
      {
        id: "menu:view",
        label: "View",
        separator: false,
        accelerator: null,
        enabled: true,
        checked: null,
        children: [
          { id: "output", label: "Output", separator: false, accelerator: null, enabled: true, checked: false, children: [] },
          { id: "push", label: "Push", separator: false, accelerator: null, enabled: true, checked: null, children: [] },
        ],
      },
    ];
    webMenus.pushState(["push"], ["output"]);
    const [view] = webMenus.bar;
    expect(view!.children.map((row) => [row.enabled, row.checked])).toEqual([
      [true, true],
      [false, null],
    ]);
  });
});
