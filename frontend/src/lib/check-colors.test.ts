import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const script = fileURLToPath(new URL("../../../scripts/check-colors.mjs", import.meta.url));

interface Guard {
  literalIn(line: string): string | null;
}
const guard = (await import(/* @vite-ignore */ new URL("file:///" + script.replaceAll("\\", "/")).href)) as Guard;

describe("the color-literal guard", () => {
  it("passes on the tree: no color literal outside themes/", () => {
    expect(execFileSync(process.execPath, [script], { encoding: "utf8" })).toContain("ok");
  });

  it.each([
    "background: #fff;",
    "color: #1f2328;",
    "fill: #12345678;",
    "box-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);",
    "color: hsl(10 20% 30%);",
    "background: color-mix(in srgb, var(--accent) 20%, transparent);",
    "border: 1px solid red;",
    "color: white;",
    "--x: black;",
    '<path fill="red" />',
    'context.fillStyle = "blue";',
  ])("flags %s", (line) => {
    expect(guard.literalIn(line)).not.toBeNull();
  });

  it.each([
    "background: var(--bg-panel);",
    "background: none;",
    "border-color: transparent;",
    "fill: currentColor;",
    "const issue = '#123 fix';",
    "{#if open}",
    "#fade: number;",
    "this.#fade = 1;",
    "// color: #fff in a comment",
    "color: var(--x, red);",
    "background: color-mix(in srgb, var(--a) 40%, var(--b));",
    "font-style: italic;",
    "transition: background var(--t-fast) var(--ease-out) !important;",
  ])("lets %s through", (line) => {
    expect(guard.literalIn(line)).toBeNull();
  });
});
