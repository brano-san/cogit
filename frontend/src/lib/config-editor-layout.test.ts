import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** Edit Git Config (#22): the editor's height came from its content, one line number per
    line, so a long .git/config squeezed the path above it; clipped by its own overflow, the
    path showed half its height. A short ~/.gitconfig fitted, and nothing gave way. */
const SOURCE = readFileSync(join(__dirname, "..", "components", "common", "ConfigEditor.svelte"), "utf8");
const AREA = readFileSync(join(__dirname, "..", "components", "common", "CodeArea.svelte"), "utf8");

function rule(selector: string, source = SOURCE): string {
  const style = source.slice(source.indexOf("<style>"));
  const match = new RegExp(`\\n\\s*${selector.replace(/\./g, "\\.")}\\s*\\{([^}]*)\\}`).exec(style);
  if (!match) throw new Error(`no rule ${selector}`);
  return match[1]!;
}

describe("config editor layout", () => {
  it("keeps the heading whole however long the config", () => {
    for (const selector of [".where", ".problem"]) expect(rule(selector)).toMatch(/flex:\s*none;/);
    // The editor takes the room left, never room its lines ask for.
    expect(rule(".editor", AREA)).toMatch(/flex:\s*1 1 0(px)?;/);
  });

  it("cuts a long path in the middle and names it whole in the tooltip", () => {
    const path = /<span[^>]*class="[^"]*\bpath\b[^"]*"[^>]*>/.exec(SOURCE)?.[0];
    expect(path).toBeDefined();
    expect(path).toContain("use:fitPath={file.path}");
    expect(SOURCE).toMatch(/title=\{file\.path\}/);
  });
});
