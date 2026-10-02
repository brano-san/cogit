import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync("src/components/layout/StartScreen.svelte", "utf8");

/** The empty state is the first child of the workspace; the panels' headers and the splitters are
    positioned and come later, so without a z-index they paint across the card (seen under WSLg). */
describe("empty state layer", () => {
  it("sits above the splitters and below menus, dialogs and the command output", () => {
    const layer = source.match(/\.layer\s*\{([^}]*)\}/)?.[1] ?? "";
    const z = Number(layer.match(/z-index:\s*(\d+)/)?.[1]);
    expect(z).toBeGreaterThan(2);
    expect(z).toBeLessThan(20);
  });
});
