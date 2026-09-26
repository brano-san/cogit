import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { parse } from "svelte/compiler";
import ts from "typescript";
import { describe, expect, it } from "vitest";

/** The window is spelled the American way (item 17 of 25.09, R-615). Read: string and
    template literals and markup text; comments and tests are not read. A literal with no
    space that starts in lower case is a key — a serde value (`notInitialised`), a class, a
    search keyword — and keeps its spelling. */
const SRC = join(__dirname, "..");

const BRITISH = new RegExp(
  "\\b(?:\\w*(?:colour|behaviour|favour|honour|neighbour|flavour|labour|humour|rumour|" +
    "centre|metre|litre|fibre|theatre|licence|defence|offence|catalogue|dialogue|programme|" +
    "judgement|artefact)\\w*|grey\\w*|whilst|amongst|learnt|spelt|" +
    "\\w*(?:cancell|labell|travell|modell|signall|levell|totall|channell|tunnell|fuell|diall|journall)(?:ed|ing|er)|" +
    "\\w+(?:is(?:e|ed|es|ing|ation|ations)|ys(?:e|ed|ing)))\\b",
  "gi",
);

/** Spelled with an s on both sides of the Atlantic. */
const AMERICAN = new Set(
  (
    "advise advised advises advising arise arises arising comprise comprised comprises comprising " +
    "compromise compromised compromises concise demise despise devise devised devises disguise " +
    "disguised enterprise enterprises excise exercise exercised exercises exercising expertise " +
    "franchise improvise improvised improvisation merchandise precise imprecise premise premises " +
    "promise promised promises raise raised raises raising revise revised revises revising rise " +
    "rises rising surprise surprised surprises surprising supervise supervised televise noise " +
    "noises poise praise praised cruise bruise chastise advertise advertised advertising " +
    "paradise reprise treatise sunrise"
  ).split(" "),
);

/** Attributes that name things for the code, not words for the user. */
const WORDS_FOR_THE_USER = new Set(["aria-label", "aria-description", "aria-roledescription", "aria-valuetext"]);
const NAMES = new Set(["class", "id", "for", "name", "type", "role", "style", "href", "src", "lang", "rel", "d"]);

function quiet(attribute: string): boolean {
  if (WORDS_FOR_THE_USER.has(attribute)) return false;
  return NAMES.has(attribute) || attribute.startsWith("aria-") || attribute.startsWith("data-");
}

function files(dir: string, found: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) files(path, found);
    else if (/\.(svelte|ts)$/.test(name) && !/\.(test|d)\.ts$/.test(name)) found.push(path);
  }
  return found;
}

function svelteTexts(source: string): string[] {
  const texts: string[] = [];
  const seen = new WeakSet<object>();
  const walk = (node: unknown): void => {
    if (!node || typeof node !== "object" || seen.has(node)) return;
    seen.add(node);
    if (Array.isArray(node)) return node.forEach(walk);
    const item = node as Record<string, unknown>;
    if (item.type === "Comment") return;
    if (item.type === "Attribute" && typeof item.name === "string" && quiet(item.name)) return;
    if (item.type === "Text" && typeof item.data === "string") texts.push(item.data);
    if (item.type === "Literal" && typeof item.value === "string") texts.push(item.value);
    if (item.type === "TemplateElement") texts.push((item.value as { cooked?: string }).cooked ?? "");
    for (const [key, value] of Object.entries(item)) {
      if (key !== "leadingComments" && key !== "trailingComments") walk(value);
    }
  };
  const ast = parse(source, { modern: true });
  walk(ast.fragment);
  walk(ast.instance);
  walk(ast.module);
  return texts;
}

function scriptTexts(source: string, path: string): string[] {
  const texts: string[] = [];
  const walk = (node: ts.Node): void => {
    if (
      ts.isStringLiteral(node) ||
      ts.isNoSubstitutionTemplateLiteral(node) ||
      ts.isTemplateHead(node) ||
      ts.isTemplateMiddle(node) ||
      ts.isTemplateTail(node)
    ) {
      texts.push(node.text);
    }
    ts.forEachChild(node, walk);
  };
  walk(ts.createSourceFile(path, source, ts.ScriptTarget.Latest, false, ts.ScriptKind.TS));
  return texts;
}

const isKey = (text: string) => !/\s/.test(text) && !/^[A-Z]/.test(text);

function britishWords(text: string): string[] {
  return [...text.matchAll(BRITISH)]
    .map((match) => match[0])
    .filter((word) => !AMERICAN.has(word.toLowerCase()) && !/wise$/i.test(word));
}

describe("spelling in the window", () => {
  it("is American in every string the user reads", () => {
    const found: string[] = [];
    for (const file of files(SRC)) {
      const source = readFileSync(file, "utf8");
      const texts = file.endsWith(".svelte") ? svelteTexts(source) : scriptTexts(source, file);
      for (const text of texts) {
        if (isKey(text)) continue;
        for (const word of britishWords(text)) {
          found.push(`${word} in ${relative(SRC, file)}: ${text.trim().slice(0, 70)}`);
        }
      }
    }
    expect(found).toEqual([]);
  }, 30_000);
});
