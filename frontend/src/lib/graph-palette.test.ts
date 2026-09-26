import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BRANCH_SLOTS, branchToken } from "$lib/graph-style";

/** The four themes as the cascade builds them: each one's blocks of app.css, in order. */
const CSS = readFileSync(join(__dirname, "..", "app.css"), "utf8");
const THEMES: Record<string, string[]> = {
  dark: [":root"],
  darkGrey: [":root", ':root[data-theme="darkGrey"]'],
  light: [":root", ':root[data-theme="light"],\n:root[data-theme="lightGrey"]'],
  lightGrey: [
    ":root",
    ':root[data-theme="light"],\n:root[data-theme="lightGrey"]',
    ':root[data-theme="lightGrey"]',
  ],
};

function block(selector: string): Map<string, string> {
  const start = CSS.indexOf(`\n${selector} {`);
  if (start < 0) throw new Error(`no block ${selector}`);
  const body = CSS.slice(CSS.indexOf("{", start) + 1, CSS.indexOf("\n}", start));
  return new Map([...body.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]));
}

function theme(name: string): (token: string) => string {
  const values = new Map<string, string>();
  for (const selector of THEMES[name]!) for (const [key, value] of block(selector)) values.set(key, value);
  const resolve = (token: string): string => {
    const value = values.get(token);
    if (value === undefined) throw new Error(`${token} is not set in ${name}`);
    const ref = /^var\((--[a-z0-9-]+)\)$/.exec(value);
    return ref ? resolve(ref[1]!) : value;
  };
  return resolve;
}

function rgb(hex: string): [number, number, number] {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error(`not a colour: ${hex}`);
  return [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16) / 255) as [number, number, number];
}

const linear = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);

function luminance(hex: string): number {
  const [r, g, b] = rgb(hex).map(linear) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2 contrast ratio. */
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** CIE76 distance in Lab: how far apart two colours look, whatever their brightness. */
function distance(a: string, b: string): number {
  const lab = (hex: string) => {
    const [r, g, b] = rgb(hex).map(linear) as [number, number, number];
    const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
    const x = f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047);
    const y = f(0.2126 * r + 0.7152 * g + 0.0722 * b);
    const z = f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883);
    return [116 * y - 16, 500 * (x - y), 200 * (y - z)];
  };
  const [p, q] = [lab(a), lab(b)];
  return Math.hypot(p[0]! - q[0]!, p[1]! - q[1]!, p[2]! - q[2]!);
}

const slots = Array.from({ length: BRANCH_SLOTS }, (_, slot) => branchToken(slot));

describe.each(Object.keys(THEMES))("branch colours in the %s theme", (name) => {
  const colour = theme(name);

  // 3:1 is WCAG's floor for graphical objects; a line crosses plain and selected rows.
  it.each(["--c-bg-panel", "--c-bg-active"])("stand out 3:1 against %s", (background) => {
    for (const token of slots) {
      expect(contrast(colour(token), colour(background)), `${token} on ${background}`).toBeGreaterThanOrEqual(3);
    }
  });

  it("are told apart from each other and from the grey of an unticked line", () => {
    for (const [i, a] of slots.entries()) {
      expect(distance(colour(a), colour("--graph-line")), `${a} against grey`).toBeGreaterThanOrEqual(25);
      for (const b of slots.slice(i + 1)) {
        expect(distance(colour(a), colour(b)), `${a} against ${b}`).toBeGreaterThanOrEqual(15);
      }
    }
  });
});

// The Files switches went back to grey under the pointer: their hover rule outranked the
// pressed one, and pressed and released looked the same (#31).
describe("a pressed switch", () => {
  it("keeps its look under the pointer: one rule for every switch, over any hover", () => {
    const body = /\n\[aria-pressed="true"\][^{]*\{([^}]*)\}/.exec(CSS)?.[1] ?? "";
    expect(body).toMatch(/background:\s*var\(--state-pressed\)\s*!important/);
    expect(body).toMatch(/color:\s*var\(--state-pressed-text\)\s*!important/);
  });

  it.each(Object.keys(THEMES))("reads 3:1 on its own background in the %s theme", (name) => {
    const colour = theme(name);
    expect(contrast(colour("--state-pressed-text"), colour("--state-pressed"))).toBeGreaterThanOrEqual(3);
    expect(colour("--state-pressed")).not.toBe(colour("--state-hover"));
  });
});

// A bisect dot sits on plain, selected and tinted rows; good and bad must differ, and a tag
// names the mark beside it for whoever cannot tell red from green (F-566).
describe.each(Object.keys(THEMES))("bisect dots in the %s theme", (name) => {
  const colour = theme(name);
  const dots = ["--graph-bisect-good", "--graph-bisect-bad", "--graph-bisect-skip"];

  it.each(["--c-bg-panel", "--c-bg-active"])("stand out 3:1 against %s", (background) => {
    for (const token of dots) {
      expect(contrast(colour(token), colour(background)), `${token} on ${background}`).toBeGreaterThanOrEqual(3);
    }
  });

  it("are told apart from each other", () => {
    for (const [i, a] of dots.entries()) {
      for (const b of dots.slice(i + 1)) {
        expect(distance(colour(a), colour(b)), `${a} against ${b}`).toBeGreaterThanOrEqual(25);
      }
    }
  });

  it("tint the rows under test and found from the theme's own warning and error", () => {
    expect(colour("--graph-bisect-current")).toBe(colour("--c-modified-bg"));
    expect(colour("--graph-bisect-found")).toBe(colour("--c-deleted-bg"));
  });
});
