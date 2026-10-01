import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/** Pure CSS-rule checks on the shared modal: no dialog may grow past the window or scroll
    sideways, whatever its content (doc/06-design-system.md, dialog rules). */
const css = readFileSync(new URL("../components/common/Dialog.svelte", import.meta.url), "utf8").split("<style>")[1] ?? "";

/** The declarations of the rule whose selector is exactly `selector`. */
function rule(selector: string): string {
  for (const block of css.split("}")) {
    const [head, body] = block.split("{");
    if (body !== undefined && head?.trim() === selector) return body;
  }
  return "";
}

describe("shared Dialog layout rules", () => {
  it("is at most 90vw by 90vh", () => {
    const dialog = rule(".dialog");
    expect(dialog).toMatch(/max-width:\s*90vw/);
    expect(dialog).toMatch(/max-height:\s*90vh/);
  });

  it("scrolls the body vertically only, with the header and footer outside it", () => {
    const body = rule(".body");
    expect(body).toMatch(/overflow-x:\s*hidden/);
    expect(body).toMatch(/overflow-y:\s*auto/);
    expect(body).toMatch(/overflow-wrap:\s*anywhere/);
    expect(rule("footer")).toMatch(/flex:\s*0 0 auto/);
    expect(rule("header")).toMatch(/flex:\s*0 0 auto/);
  });

  it("lets rows shrink: body children and the footer have min-width 0", () => {
    expect(rule(".body > :global(*)")).toMatch(/min-width:\s*0/);
    expect(rule("footer")).toMatch(/min-width:\s*0/);
  });

  it("keeps text fields inside their parent", () => {
    expect(css).toMatch(/input\[type="text"\][\s\S]*?max-width:\s*100%/);
  });
});
