import schemaFile from "../themes/tokens.schema.json";
import light from "../themes/light.json";
import lightGray from "../themes/light-gray.json";
import darkGray from "../themes/dark-gray.json";
import dark from "../themes/dark.json";
import type { Theme } from "./settings";

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

export const THEME_FILES: Record<Theme, ThemeFile> = {
  light: light as ThemeFile,
  lightGrey: lightGray as ThemeFile,
  darkGrey: darkGray as ThemeFile,
  dark: dark as ThemeFile,
};

/** What a family falls back to. */
const BASE: Record<ThemeFile["family"], Theme> = { light: "light", dark: "dark" };

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
  theme: Theme,
  user?: unknown,
  files: Record<Theme, ThemeFile> = THEME_FILES,
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
export function applyVariables(root: ThemeRoot, theme: Theme, vars: Record<string, string>): void {
  root.dataset["theme"] = theme;
  for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
}

export function isTheme(value: unknown): value is Theme {
  return typeof value === "string" && Object.hasOwn(THEME_FILES, value);
}
