import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, type Settings } from "./settings";
import {
  CATEGORIES,
  disabledBy,
  fieldDisabled,
  isSetting,
  firstMatch,
  matchingCategories,
  restoreCategory,
  restoreKeys,
  changedKeys,
  sameKeymap,
} from "./preferences";

const ids = CATEGORIES.map((category) => category.id);

describe("CATEGORIES", () => {
  it("groups the settings under the five top-level sections plus the keymap", () => {
    const roots = CATEGORIES.filter((category) => category.parent === undefined);
    expect(roots.map((category) => category.title)).toEqual([
      "Commands",
      "User Interface",
      "Diff & Merge",
      "Tools & Integrations",
      "Advanced",
    ]);
  });

  it("gives every category a distinct id", () => {
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("points every child at a parent that exists", () => {
    for (const category of CATEGORIES) {
      if (category.parent === undefined) continue;
      expect(ids).toContain(category.parent);
    }
  });

  it("covers every setting exactly once", () => {
    const placed = CATEGORIES.flatMap((category) =>
      category.groups.flatMap((group) => group.fields.map((field) => field.key)),
    ).filter(isSetting);
    const keys = Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[];

    expect([...placed].sort()).toEqual([...keys].sort());
  });

  // Requirement 1.6: every "Don't show again" can be taken back from one place.
  it("puts the list of hidden dialogs somewhere the user can reach it", () => {
    const found = CATEGORIES.filter((category) =>
      category.groups.some((group) => group.fields.some((field) => field.key === "suppressions")),
    );
    expect(found).toHaveLength(1);
  });

  it("puts the keymap somewhere the user can reach it", () => {
    const withKeymap = CATEGORIES.filter((category) =>
      category.groups.some((group) => group.fields.some((field) => field.key === "keymap")),
    );
    expect(withKeymap).toHaveLength(1);
  });
});

describe("matchingCategories", () => {
  it("returns every category when nothing is typed", () => {
    expect(matchingCategories("")).toEqual(ids);
  });

  it("finds a category by a field's label", () => {
    expect(matchingCategories("lane width")).toContain("graph");
  });

  it("finds a category by a word the label does not use", () => {
    expect(matchingCategories("shortcut")).toContain("keymap");
  });

  it("keeps the parent of a child that matched, so the tree still shows a path", () => {
    const found = matchingCategories("lane width");
    expect(found).toContain("ui");
  });

  it("returns nothing for a word that appears nowhere", () => {
    expect(matchingCategories("zzzznothing")).toEqual([]);
  });
});

describe("firstMatch", () => {
  it("picks a leaf, never a heading", () => {
    const picked = firstMatch("lane width");
    expect(picked).toBe("graph");
  });

  it("is null when nothing matched", () => {
    expect(firstMatch("zzzznothing")).toBeNull();
  });
});

describe("restoreCategory", () => {
  it("puts back the defaults of that category alone", () => {
    const draft: Settings = { ...DEFAULT_SETTINGS, laneWidth: 30, contextLines: 12 };
    const restored = restoreCategory(draft, "graph");

    expect(restored.laneWidth).toBe(DEFAULT_SETTINGS.laneWidth);
    expect(restored.contextLines).toBe(12);
  });

  it("leaves the draft alone for a category it does not know", () => {
    const draft: Settings = { ...DEFAULT_SETTINGS, laneWidth: 30 };
    expect(restoreCategory(draft, "nonsense")).toEqual(draft);
  });
});

describe("changedKeys", () => {
  it("says nothing changed when nothing did", () => {
    expect(changedKeys({ ...DEFAULT_SETTINGS }, { ...DEFAULT_SETTINGS })).toEqual([]);
  });

  it("names the key that moved", () => {
    const draft = { ...DEFAULT_SETTINGS, theme: "light" as const };
    expect(changedKeys(draft, DEFAULT_SETTINGS)).toEqual(["theme"]);
  });

  it("names every key that moved", () => {
    const draft = { ...DEFAULT_SETTINGS, theme: "light" as const, contextLines: 9 };
    expect(changedKeys(draft, DEFAULT_SETTINGS).sort()).toEqual(["contextLines", "theme"]);
  });
});

describe("sameKeymap", () => {
  it("ignores the order the keys were added in", () => {
    // `JSON.stringify` would call these two different, and the OK button would light up
    // on a dialog where nothing was touched.
    expect(sameKeymap({ a: "Ctrl+A", b: "Ctrl+B" }, { b: "Ctrl+B", a: "Ctrl+A" })).toBe(true);
  });

  it("sees a changed binding", () => {
    expect(sameKeymap({ a: "Ctrl+A" }, { a: "Ctrl+X" })).toBe(false);
  });

  it("sees an added binding", () => {
    expect(sameKeymap({ a: "Ctrl+A" }, { a: "Ctrl+A", b: "Ctrl+B" })).toBe(false);
  });

  it("sees a removed binding", () => {
    expect(sameKeymap({ a: "Ctrl+A", b: "Ctrl+B" }, { a: "Ctrl+A" })).toBe(false);
  });

  it("calls two empty keymaps the same", () => {
    expect(sameKeymap({}, {})).toBe(true);
  });
});

describe("options that depend on another option", () => {
  const every = CATEGORIES.flatMap((category) =>
    category.groups.flatMap((group) => group.fields),
  );

  it("names a real setting as the parent, never a typo", () => {
    for (const field of every.filter((f) => f.dependsOn)) {
      expect(every.some((other) => other.key === field.dependsOn)).toBe(true);
    }
  });

  // The reported case: the second tick sat visually under the first and stayed live
  // when the first was cleared.
  it("makes intra-line highlighting depend on move detection", () => {
    const word = every.find((field) => field.key === "wordDiff");
    expect(word?.dependsOn).toBe("detectMoves");
  });

  it("says a dependent option is off when its parent is", () => {
    expect(disabledBy({ ...DEFAULT_SETTINGS, detectMoves: false }, "detectMoves")).toBe(true);
    expect(disabledBy({ ...DEFAULT_SETTINGS, detectMoves: true }, "detectMoves")).toBe(false);
  });

  it("leaves an option with no parent alone", () => {
    expect(disabledBy(DEFAULT_SETTINGS, undefined)).toBe(false);
  });

  it("treats a non-boolean parent as satisfied, since there is nothing to switch off", () => {
    expect(disabledBy(DEFAULT_SETTINGS, "theme")).toBe(false);
  });

  it("switches the time format off while the Time column is hidden (#23)", () => {
    const format = every.find((field) => field.key === "graphTimeFormat");
    expect(format).toBeDefined();
    if (!format) return;
    expect(fieldDisabled(DEFAULT_SETTINGS, format)).toBe(false);
    expect(fieldDisabled({ ...DEFAULT_SETTINGS, graphColumns: ["author", "hash"] }, format)).toBe(
      true,
    );
  });
});

describe("the graph page (#23)", () => {
  const graph = CATEGORIES.find((category) => category.id === "graph");
  const keys = graph?.groups.flatMap((group) => group.fields.map((field) => field.key)) ?? [];

  it("holds every graph display key and every graph mode", () => {
    const graphKeys = Object.keys(DEFAULT_SETTINGS).filter((key) => key.startsWith("graph"));
    for (const key of graphKeys) expect(keys).toContain(key);
  });

  it("restores the columns and the modes with the rest of the page", () => {
    const draft: Settings = {
      ...DEFAULT_SETTINGS,
      graphColumns: ["hash"],
      graphFirstParent: true,
      graphLongLinkRows: 0,
    };
    const restored = restoreCategory(draft, "graph");
    expect(restored.graphColumns).toEqual(DEFAULT_SETTINGS.graphColumns);
    expect(restored.graphFirstParent).toBe(false);
    expect(restored.graphLongLinkRows).toBe(DEFAULT_SETTINGS.graphLongLinkRows);
  });

  it("does not call an equal column list a change", () => {
    const draft = { ...DEFAULT_SETTINGS, graphColumns: [...DEFAULT_SETTINGS.graphColumns] };
    expect(changedKeys(draft, DEFAULT_SETTINGS)).toEqual([]);
    expect(changedKeys({ ...draft, graphColumns: ["hash"] }, DEFAULT_SETTINGS)).toEqual([
      "graphColumns",
    ]);
  });
});

describe("the Toolbar page (#24)", () => {
  it("sits under User Interface and holds the toolbar editor", () => {
    const page = CATEGORIES.find((category) => category.id === "toolbar");
    expect(page?.parent).toBe("ui");
    expect(page?.groups.flatMap((group) => group.fields.map((field) => field.key))).toEqual([
      "toolbar",
    ]);
  });

  it("is found by the words of the old menu item", () => {
    expect(firstMatch("configure")).toBe("toolbar");
    expect(matchingCategories("toolbar")).toContain("toolbar");
  });

  it("is not a setting of the settings file: the layout lives with the toolbar", () => {
    expect(isSetting("toolbar")).toBe(false);
    const draft: Settings = { ...DEFAULT_SETTINGS, laneWidth: 30 };
    expect(restoreCategory(draft, "toolbar")).toEqual(draft);
  });
});

// Restore Defaults on the Keyboard page skipped the keymap: the shortcuts stayed, and a
// half-made draft was applied instead.
describe("restoreKeys", () => {
  const edited = { fetch: "CmdOrCtrl+Shift+G" };

  it("clears every override on the page that holds the keymap", () => {
    const page = CATEGORIES.find((category) => category.groups.some((group) => group.fields.some((field) => field.key === "keymap")));
    expect(restoreKeys(edited, page!.id)).toEqual({});
  });

  it("leaves the keymap alone on any other page", () => {
    expect(restoreKeys(edited, "graph")).toEqual(edited);
  });
});
