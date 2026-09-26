import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** The footer's buttons sit on its middle line (#18). The ⚠ was a character, drawn by
    whichever fallback font had it, with that font's height and baseline: the triangle and
    the count rode above the centre of the bar and of their own box. */
const SOURCE = readFileSync(join(__dirname, "..", "components", "layout", "StatusBar.svelte"), "utf8");
const APP_CSS = readFileSync(join(__dirname, "..", "app.css"), "utf8");

function rule(selector: string): string {
  const style = SOURCE.slice(SOURCE.indexOf("<style>"));
  const match = new RegExp(`\\n\\s*${selector.replace(/\./g, "\\.")}\\s*\\{([^}]*)\\}`).exec(style);
  if (!match) throw new Error(`no rule ${selector}`);
  return match[1]!;
}

function px(value: string | undefined): number {
  const match = /^(\d+(?:\.\d+)?)px$/.exec(value?.trim() ?? "");
  if (!match) throw new Error(`not a length in px: ${value}`);
  return Number(match[1]);
}

describe("status bar layout", () => {
  it("draws the problems mark as an icon, not as a font glyph", () => {
    const button = /<button[^>]*class="[^"]*\bproblems\b[^"]*"[\s\S]*?<\/button>/.exec(SOURCE)?.[0];
    expect(button).toBeDefined();
    expect(button).toContain("<svg");
    expect(button).not.toMatch(/[☀-➿]/);
  });

  it("centres every footer button's content in a box no taller than the bar", () => {
    const buttons = [...SOURCE.matchAll(/<button[^>]*class="([^"]*)"/g)].map((match) => match[1]!);
    expect(buttons.length).toBeGreaterThanOrEqual(2);
    for (const classes of buttons) expect(classes.split(" ")).toContain("foot-button");

    const body = rule(".foot-button");
    expect(body).toMatch(/display:\s*inline-flex;/);
    expect(body).toMatch(/align-items:\s*center;/);
    const bar = px(/--h-statusbar:\s*([^;]+);/.exec(APP_CSS)?.[1]);
    const height = px(/(?:^|[;\s])height:\s*([^;]+);/.exec(body)?.[1]);
    // The bar's top border is inside its height.
    expect(height).toBeLessThanOrEqual(bar - 1);
  });
});
