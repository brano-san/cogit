import type { ContextItem, SpellLanguage } from "./ipc";
import { SEPARATOR, item } from "./context-menu";
import { MENU_PREFIX } from "./solver-menu";

export interface SpellWord {
  start: number;
  end: number;
  word: string;
}

const EDGE_PUNCTUATION = /^[\p{P}\p{S}]+|[\p{P}\p{S}]+$/gu;
const PROSE = /^[\p{L}\p{M}'’-]+$/u;
const PART = /[^-]+/g;
const TRAILER = /^[A-Za-z][\w-]*: .*<[^<>]+@[^<>]+>\s*$/;

function blanked(text: string): string {
  let out = "";
  let fenced = false;
  for (const line of text.split("\n")) {
    const fence = line.trimStart().startsWith("```");
    const skipped = fenced || fence || line.startsWith("#") || TRAILER.test(line);
    if (fence) fenced = !fenced;
    out += (skipped ? " ".repeat(line.length) : line.replace(/`[^`]*`/g, (code) => " ".repeat(code.length))) + "\n";
  }
  return out;
}

function isProse(part: string): boolean {
  const letters = part.replace(/['’]/g, "");
  return letters.length > 1 && letters.slice(1) === letters.slice(1).toLowerCase();
}

export function spellWords(text: string): SpellWord[] {
  const found: SpellWord[] = [];
  for (const chunk of blanked(text).matchAll(/\S+/g)) {
    const raw = chunk[0];
    const lead = raw.length - raw.replace(/^[\p{P}\p{S}]+/u, "").length;
    const core = raw.replace(EDGE_PUNCTUATION, "");
    if (!core || !PROSE.test(core)) continue;
    for (const part of core.matchAll(PART)) {
      if (!isProse(part[0])) continue;
      const start = chunk.index + lead + part.index;
      found.push({ start, end: start + part[0].length, word: part[0] });
    }
  }
  return found;
}

function escaped(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

export function mirrorHtml(text: string, words: readonly SpellWord[], misses: ReadonlySet<string>): string {
  let out = "";
  let at = 0;
  for (const found of words) {
    if (!misses.has(found.word)) continue;
    out += `${escaped(text.slice(at, found.start))}<span class="miss" data-at="${found.start}">${escaped(found.word)}</span>`;
    at = found.end;
  }
  return `${out}${escaped(text.slice(at))}\n`;
}

export function spellLanguages(settings: { spellEnUs: boolean; spellRuRu: boolean }): SpellLanguage[] {
  const languages: SpellLanguage[] = [];
  if (settings.spellEnUs) languages.push("en_US");
  if (settings.spellRuRu) languages.push("ru_RU");
  return languages;
}

export const SPELL_USE = "spell-use-";
export const SPELL_ADD = "spell-add";

export function spellMenu(label: string | null, word: string, suggestions: readonly string[]): ContextItem[] {
  const id = (action: string) => (label === null ? action : `${MENU_PREFIX}${label}:${action}`);
  const offered =
    suggestions.length > 0
      ? suggestions.map((suggestion, at) => item(id(`${SPELL_USE}${at}`), suggestion))
      : [item("", "No Suggestions", false)];
  return [...offered, SEPARATOR, item(id(SPELL_ADD), `Add "${word}" to Dictionary`)];
}

export function knownWords(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  return [...new Set(value.filter((entry): entry is string => typeof entry === "string").map((entry) => entry.trim()))].filter(
    (entry) => entry !== "",
  );
}

export function accepted(word: string, personal: readonly string[]): boolean {
  const lower = word.toLowerCase();
  return personal.some((entry) => entry.toLowerCase() === lower);
}
