import { describe, expect, it } from "vitest";
import { SEPARATOR, item, offer, refMenu, submenu, tidy, withKeys } from "./context-menu";

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

  // #19 of 25.09: every row of Branches has Toggle, a lost commit too.
  it("offers a lost commit the way back and its box", () => {
    expect(ids(refMenu({ kind: "lost" }))).toEqual(["restore-lost", "lost-copy-sha", "lost-toggle"]);
  });

  it("has nothing to offer for a heading, which has a menu of its own", () => {
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

describe("tidy at every depth", () => {
  it("tidies the children of a submenu given as a plain row", () => {
    const nested = { ...item("move", "Move To"), children: [SEPARATOR, item("a", "A"), SEPARATOR, SEPARATOR, item("b", "B"), SEPARATOR] };
    const [row] = tidy([nested]);
    expect(row!.children!.map((child) => (child.separator ? "-" : child.id))).toEqual(["a", "-", "b"]);
  });

  it("keeps a submenu that has nothing inside as a row that is off", () => {
    const empty = { ...item("move", "Move To"), children: [SEPARATOR, SEPARATOR] };
    const [row] = tidy([item("x", "X"), empty].slice(1));
    expect(row!.enabled).toBe(false);
    expect(row!.children).toEqual([]);
  });
});

describe("withKeys", () => {
  const keys = { stage: "CmdOrCtrl+T", unstage: "", branch: "F7" };

  it("shows the key of the command a row runs, as the user set it", () => {
    const [stage, unstage, other] = withKeys([item("file-stage", "Stage"), item("file-unstage", "Unstage"), item("file-log", "Log")], keys);
    expect(stage!.accelerator).toBe("CmdOrCtrl+T");
    expect(unstage!.accelerator).toBeNull();
    expect(other!.accelerator).toBeNull();
  });

  it("keeps a key a row was given and reaches into submenus", () => {
    const [own, nested] = withKeys([item("file-stage", "Stage", true, "Space"), submenu("more", "More", [item("ref:add-branch", "Add Branch")])], keys);
    expect(own!.accelerator).toBe("Space");
    expect(nested!.children![0]!.accelerator).toBe("F7");
  });

  it("names fixed panel keys without a keymap", () => {
    expect(withKeys([item("file-discard", "Discard…")], {})[0]!.accelerator).toBe("CmdOrCtrl+Z");
  });
});
