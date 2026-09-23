import { describe, expect, it } from "vitest";
import { SEPARATOR, item, offer, refMenu, submenu, tidy } from "./context-menu";

describe("tidy", () => {
  const sep = SEPARATOR;
  const a = item("a", "A");
  const b = item("b", "B");

  it("drops leading, trailing and doubled separators", () => {
    expect(tidy([sep, a, sep, sep, b, sep])).toEqual([a, sep, b]);
  });

  it("leaves nothing of a menu that is only separators", () => {
    expect(tidy([sep, sep])).toEqual([]);
  });
});

describe("offer", () => {
  it("is an ordinary row when nothing blocks it", () => {
    expect(offer("x", "Squash", null)).toEqual(item("x", "Squash"));
  });

  it("puts the reason into the label of a row that is off", () => {
    const off = offer("x", "Squash", "already pushed", "CmdOrCtrl+Q");
    expect(off.enabled).toBe(false);
    expect(off.label).toBe("Squash (already pushed)");
    expect(off.accelerator).toBe("CmdOrCtrl+Q");
  });
});

describe("refMenu", () => {
  const ids = (items: ReturnType<typeof refMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);

  it("offers a lost commit the way back", () => {
    expect(ids(refMenu({ kind: "lost" }))).toEqual(["restore-lost", "copy-sha"]);
  });

  it("has nothing to offer for a heading", () => {
    expect(refMenu({ kind: "group" })).toEqual([]);
  });
});

describe("submenu", () => {
  it("carries its children, tidied like a menu of their own", () => {
    const menu = submenu("move", "Move To", [SEPARATOR, item("a", "A"), SEPARATOR, SEPARATOR, item("b", "B")]);
    expect(menu.children?.map((child) => (child.separator ? "-" : child.id))).toEqual(["a", "-", "b"]);
    expect(menu.enabled).toBe(true);
  });

  it("is off when told so, and when nothing is left inside", () => {
    expect(submenu("r", "Resolve", [item("t", "Take Theirs")], false).enabled).toBe(false);
    expect(submenu("r", "Resolve", [SEPARATOR]).enabled).toBe(false);
  });
});
