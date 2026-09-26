import { describe, expect, it } from "vitest";
import { graphOptions } from "./graph-options";
import { DEFAULT_SETTINGS } from "./settings";

describe("graphOptions", () => {
  it("lists the colorings first, one of them chosen: the one in the settings", () => {
    const menu = graphOptions({ ...DEFAULT_SETTINGS, graphColoring: "mergeable" });
    const colorings = menu.filter((entry) => entry.kind === "coloring");
    expect(colorings.map((entry) => entry.label)).toEqual([
      "Default Coloring",
      "Branch Coloring",
      "Mergeable Coloring",
      "Varying Coloring",
    ]);
    expect(colorings.filter((entry) => entry.checked).map((entry) => entry.coloring)).toEqual(["mergeable"]);
  });

  it("links to the graph's page of Preferences between separators", () => {
    const menu = graphOptions(DEFAULT_SETTINGS);
    const at = menu.findIndex((entry) => entry.kind === "preferences");
    expect(menu[at - 1]?.kind).toBe("separator");
    expect(menu[at + 1]?.kind).toBe("separator");
  });

  it("shows each switch as its setting has it", () => {
    const on = graphOptions({ ...DEFAULT_SETTINGS, graphFirstParent: true });
    expect(on.find((entry) => entry.kind === "switch" && entry.key === "graphFirstParent")).toMatchObject({
      label: "Follow Only First Parent",
      checked: true,
    });
  });

  it("neither starts nor ends with a separator, nor doubles one", () => {
    const menu = graphOptions(DEFAULT_SETTINGS);
    expect(menu[0]?.kind).not.toBe("separator");
    expect(menu.at(-1)?.kind).not.toBe("separator");
    menu.forEach((entry, index) => {
      if (entry.kind === "separator") expect(menu[index + 1]?.kind).not.toBe("separator");
    });
  });
});
