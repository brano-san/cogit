import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, type Settings } from "./settings";
import {
  CATEGORIES,
  firstMatch,
  matchingCategories,
  restoreCategory,
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
    ).filter((key): key is keyof Settings => key !== "keymap");
    const keys = Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[];

    expect([...placed].sort()).toEqual([...keys].sort());
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
