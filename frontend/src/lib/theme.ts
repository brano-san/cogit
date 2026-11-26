import schemaFile from "../themes/tokens.schema.json";
import light from "../themes/light.json";
import lightGray from "../themes/light-gray.json";
import darkGray from "../themes/dark-gray.json";
import graphite from "../themes/graphite.json";
import dark from "../themes/dark.json";
import type { FixedTheme, Theme } from "./settings";

/** The color layer: `themes/*.json` are the only place a color literal may live. A token is
    a dotted name (`diff.add.word`); it becomes the custom property `--diff-add-word`.
    doc/06-design-system.md says how to add one. */

export type TokenType = "color" | "color-alpha" | "string";
export type TokenMap = Record<string, string>;

export interface ThemeFile {
  name: string;
  /** The base theme this one falls back to for a token it lacks. */
  family: "light" | "dark";
  tokens: TokenMap;
}

export const SCHEMA = schemaFile.tokens as Record<string, { type: TokenType; description: string }>;

export const THEME_FILES: Record<FixedTheme, ThemeFile> = {
  light: light as ThemeFile,
  lightGrey: lightGray as ThemeFile,
  darkGrey: darkGray as ThemeFile,
  graphite: graphite as ThemeFile,
  dark: dark as ThemeFile,
};

/** What a family falls back to. */
const BASE: Record<ThemeFile["family"], FixedTheme> = { light: "light", dark: "dark" };

export { THEME_CACHE_KEY } from "./theme-cache";

/** `bg.selectedInactive` → `--bg-selected-inactive`: dots become dashes, camelCase is kebab-cased. */
export function cssName(token: string): string {
  return `--${token.replace(/([a-z0-9])([A-Z])/g, "$1-$2").replace(/\./g, "-").toLowerCase()}`;
}

const SOLID = /^#[0-9a-f]{6}$/i;
const WITH_ALPHA = /^#[0-9a-f]{6}([0-9a-f]{2})?$/i;

export function validValue(type: TokenType, value: unknown): value is string {
  if (typeof value !== "string") return false;
  if (type === "string") return value.length > 0;
  return (type === "color" ? SOLID : WITH_ALPHA).test(value);
}

export interface Resolved {
  tokens: TokenMap;
  warnings: string[];
}

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

/** The tokens of `theme`: its own file, a missing or malformed one taken from the base theme
    of its family (with a warning), then the user's `user-theme.json` over the top. */
export function resolveTheme(
  theme: FixedTheme,
  user?: unknown,
  files: Record<FixedTheme, ThemeFile> = THEME_FILES,
  schema: Record<string, { type: TokenType }> = SCHEMA,
): Resolved {
  const warnings: string[] = [];
  const own = files[theme];
  const base = files[BASE[own.family]];
  const tokens: TokenMap = {};
  for (const [name, { type }] of Object.entries(schema)) {
    const value = own.tokens[name];
    if (validValue(type, value)) {
      tokens[name] = value;
      continue;
    }
    const reason = value === undefined ? "is missing" : `has an invalid value ${JSON.stringify(value)}`;
    const fallback = base.tokens[name];
    if (validValue(type, fallback)) {
      tokens[name] = fallback;
      warnings.push(`theme ${theme}: token ${name} ${reason}; using ${BASE[own.family]}`);
    } else {
      warnings.push(`theme ${theme}: token ${name} ${reason} and so is the base theme's; left unset`);
    }
  }
  if (user !== undefined) mergeUser(tokens, user, schema, warnings);
  return { tokens, warnings };
}

/** `{ "tokens": { "accent": "<color>" } }`: only valid keys of the schema count. */
function mergeUser(
  tokens: TokenMap,
  user: unknown,
  schema: Record<string, { type: TokenType }>,
  warnings: string[],
): void {
  if (!isObject(user) || !isObject(user["tokens"])) {
    if (!(isObject(user) && Object.keys(user).length === 0)) {
      warnings.push('user-theme.json: expected {"tokens": {…}}; ignored');
    }
    return;
  }
  for (const [name, value] of Object.entries(user["tokens"])) {
    const entry = schema[name];
    if (!entry) warnings.push(`user-theme.json: unknown token ${name}; ignored`);
    else if (!validValue(entry.type, value)) {
      warnings.push(`user-theme.json: token ${name} has an invalid value ${JSON.stringify(value)}; ignored`);
    } else tokens[name] = value;
  }
}

const channels = (hex: string) => [1, 3, 5].map((offset) => Number.parseInt(hex.slice(offset, offset + 2), 16));

function mix(from: string, to: string, weight: number): string {
  const target = channels(to);
  const mixed = channels(from).map((channel, i) => Math.round(channel + (target[i]! - channel) * weight));
  return `#${mixed.map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;
}

function luminance(hex: string): number {
  const weights = [0.2126, 0.7152, 0.0722];
  return channels(hex).reduce((sum, channel, i) => {
    const srgb = channel / 255;
    const linear = srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
    return sum + linear * weights[i]!;
  }, 0);
}

function readable(color: string, backgrounds: string[], toward: string, min: number): string {
  const levels = backgrounds.map(luminance);
  const fits = (hex: string) => {
    const level = luminance(hex);
    return levels.every((background) => (Math.max(level, background) + 0.05) / (Math.min(level, background) + 0.05) >= min);
  };
  if (fits(color)) return color;
  let lo = 0;
  let hi = 1;
  for (let step = 0; step < 8; step++) {
    const weight = (lo + hi) / 2;
    if (fits(mix(color, toward, weight))) hi = weight;
    else lo = weight;
  }
  return mix(color, toward, hi);
}

/** Recolor accents while preserving the status palette and legible text on small controls. */
export function applyAccent(tokens: TokenMap, color: string, theme: FixedTheme): void {
  const dark = isDarkTheme(theme);
  const defaults = THEME_FILES[theme].tokens;
  const builtin = (name: string) => defaults[name]!;
  const token = (name: string) => tokens[name] ?? builtin(name);
  const ink = builtin("fg.primary");
  const panel = token("bg.panel");
  const selection = mix(panel, color, dark ? 0.16 : 0.11);
  const accent = readable(color, [panel, token("bg.app"), token("bg.editor"), token("bg.hover"),
    token("bg.elevated"), token("bg.input"), token("fg.onAccent"), selection], ink, 4.5);
  const solid = readable(color, [token("fg.onAccentSolid")], THEME_FILES.light.tokens["fg.primary"]!, 5.3);
  Object.assign(tokens, {
    accent,
    "accent.hover": mix(accent, ink, 0.15),
    "accent.solid": solid,
    "bg.selected": selection,
    "selected.bar": accent,
    "panel.activeHeader": accent,
    "focus.ring": `${accent}${dark ? "80" : "66"}`,
    "toggle.on.bg": selection,
    "toggle.on.fg": accent,
    "toggle.on.border": accent,
  });
}

/** The custom properties of a token map; a `string` token is not a CSS value. */
export function cssVariables(
  tokens: TokenMap,
  schema: Record<string, { type: TokenType }> = SCHEMA,
): Record<string, string> {
  const vars: Record<string, string> = {};
  for (const [name, value] of Object.entries(tokens)) {
    if (schema[name]?.type !== "string") vars[cssName(name)] = value;
  }
  return vars;
}

export interface ThemeRoot {
  dataset: Record<string, string | undefined>;
  style: { setProperty(name: string, value: string): void };
}

/** Inline custom properties on `:root`: a swap is these lines again, no reload. */
export function applyVariables(root: ThemeRoot, theme: FixedTheme, vars: Record<string, string>): void {
  root.dataset["theme"] = theme;
  // The chosen theme, never the OS preference: native form controls, scrollbars and the
  // default canvas of the web view (WebView2 and WebKitGTK alike) follow it.
  root.style.setProperty("color-scheme", isDarkTheme(theme) ? "dark" : "light");
  for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
}

export function isTheme(value: unknown): value is Theme {
  return value === "system" || (typeof value === "string" && Object.hasOwn(THEME_FILES, value));
}

/** Whether a theme is dark, for what the app does not draw itself: the native toolkit
    (`GtkSettings` prefer-dark) and the web view's `color-scheme`. Family, not name. */
export function isDarkTheme(theme: FixedTheme, files: Record<FixedTheme, ThemeFile> = THEME_FILES): boolean {
  return files[theme].family === "dark";
}
