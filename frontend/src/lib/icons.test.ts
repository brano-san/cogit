import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { parse } from "svelte/compiler";
import { describe, expect, it } from "vitest";
import { GLYPHS, type Glyph } from "./icons";

const SRC = join(__dirname, "..");

const TEXT_ICONS = /[✕✖✗✓✔⋯▲▼▶◀‹›↑↓🔒]/u;

function svelteFiles(dir: string, found: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) svelteFiles(path, found);
    else if (name.endsWith(".svelte")) found.push(path);
  }
  return found;
}

function markupTexts(source: string): string[] {
  const texts: string[] = [];
  const seen = new WeakSet<object>();
  const walk = (node: unknown): void => {
    if (!node || typeof node !== "object" || seen.has(node)) return;
    seen.add(node);
    if (Array.isArray(node)) return node.forEach(walk);
    const item = node as Record<string, unknown>;
    if (item.type === "Comment" || item.type === "Attribute") return;
    if (item.type === "Text" && typeof item.data === "string") texts.push(item.data);
    if (item.type === "Literal" && typeof item.value === "string") texts.push(item.value);
    for (const [key, value] of Object.entries(item)) {
      if (key !== "leadingComments" && key !== "trailingComments") walk(value);
    }
  };
  walk(parse(source, { modern: true }).fragment);
  return texts;
}

describe("icons", () => {
  it("uses the repository glyph for submodules without a dedicated glyph", () => {
    expect(GLYPHS).toHaveProperty("repository");
    expect(GLYPHS).not.toHaveProperty("submodule");
  });

  it("gives every glyph a shape of its own", () => {
    const owners = new Map<string, string>();
    const shared: string[] = [];
    for (const [name, glyph] of Object.entries(GLYPHS) as [string, Glyph][]) {
      const shape = `${glyph.grid}|${glyph.d}|${glyph.line ?? ""}`;
      const owner = owners.get(shape);
      if (owner) shared.push(`${owner} = ${name}`);
      else owners.set(shape, name);
    }
    expect(shared).toEqual([]);
  });

  it("draws every glyph from a move", () => {
    for (const glyph of Object.values(GLYPHS) as Glyph[]) {
      expect(glyph.d).toMatch(/^[Mm]/);
      if (glyph.line !== undefined) expect(glyph.line).toMatch(/^[Mm]/);
    }
  });

  it("keeps symbols out of the markup text: icons come from the glyph map", () => {
    const found: string[] = [];
    for (const file of svelteFiles(SRC)) {
      for (const text of markupTexts(readFileSync(file, "utf8"))) {
        if (TEXT_ICONS.test(text)) found.push(`${relative(SRC, file)}: ${text.trim().slice(0, 60)}`);
      }
    }
    expect(found).toEqual([]);
  }, 30_000);
});
