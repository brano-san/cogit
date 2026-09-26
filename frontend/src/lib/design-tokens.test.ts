import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/** A misspelt token is not an error anywhere: the declaration is dropped and the element
    quietly loses its spacing or its colour. Two dialogs shipped that way (R-152). */
const SRC = join(__dirname, "..");

function files(dir: string, found: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) files(path, found);
    else if (name.endsWith(".svelte") || name.endsWith(".css")) found.push(path);
  }
  return found;
}

function declared(text: string): Set<string> {
  return new Set([...text.matchAll(/(--[a-z0-9-]+)\s*:/g)].map((match) => match[1]!));
}

describe("design tokens", () => {
  const global = declared(readFileSync(join(SRC, "app.css"), "utf8"));

  it("uses no custom property that is declared nowhere", () => {
    const missing: string[] = [];
    for (const file of files(SRC)) {
      const text = readFileSync(file, "utf8");
      const local = declared(text);
      // `style:--name={…}` sets a property inline, which is a declaration too.
      for (const match of text.matchAll(/style:(--[a-z0-9-]+)/g)) local.add(match[1]!);
      for (const match of text.matchAll(/var\((--[a-z0-9-]+)/g)) {
        const token = match[1]!;
        if (!global.has(token) && !local.has(token)) missing.push(`${token} in ${file.slice(SRC.length)}`);
      }
    }
    expect([...new Set(missing)]).toEqual([]);
  });

  // The LFS download link and "Use inherited" took the branch colour: one link in About,
  // another in Repository Settings (F-278).
  it("gives every link one colour", () => {
    const astray: string[] = [];
    for (const file of files(SRC)) {
      const text = readFileSync(file, "utf8");
      for (const [, selector, body] of text.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
        const link = /\.link\b/.test(selector!) || /text-decoration:\s*underline/.test(body!);
        const colour = /(?:^|[;\s])color:\s*([^;]+);/.exec(body!)?.[1]?.trim();
        if (link && colour && colour !== "var(--link)") {
          astray.push(`${selector!.trim()} in ${file.slice(SRC.length)}: ${colour}`);
        }
      }
    }
    expect(astray).toEqual([]);
  });
});
