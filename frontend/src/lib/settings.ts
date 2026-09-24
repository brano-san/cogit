import type { DateMode } from "$lib/format";
import type { Algorithm, Whitespace } from "$lib/ipc";

/** Lightest first; the grey ones sit between the extremes (#24). */
export const THEMES = [
  ["light", "Light"],
  ["lightGrey", "Light grey"],
  ["darkGrey", "Dark grey"],
  ["dark", "Dark"],
] as const;

export type Theme = (typeof THEMES)[number][0];

export interface Settings {
  theme: Theme;
  dateFormat: DateMode;
  algorithm: Algorithm;
  contextLines: number;
  ignoreWhitespace: Whitespace;
  wordDiff: boolean;
  detectMoves: boolean;
  laneWidth: number;
  /** A colour per lane instead of one grey; off, as SmartGit draws it (R-161). */
  coloredLanes: boolean;
  pullMode: "ffOnly" | "merge";
  gitPath: string;
  terminal: string;
  logLevel: "error" | "warn" | "info" | "debug" | "trace";
  /** On by default: a face per row is what the commit list is read by, and the request
      carries a hash rather than the address (doc/12-risks.md, R-145). `ask` is what a
      settings file written before that default said. */
  avatars: "ask" | "gravatar" | "off";
  /** Off until ticked: nothing reaches the network unasked. Help ▸ Check for Updates…
      works either way. */
  autoUpdate: boolean;
  /** The Exit dialog's "Don't show again" and this checkbox are the same value (R-151). */
  confirmExit: boolean;
  /** Minutes between asking the remote of every listed repository what it has (R-354);
      `0` is off, which is the default: nothing reaches the network unasked. */
  backgroundFetchMinutes: number;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "dark",
  dateFormat: "smart",
  algorithm: "histogram",
  contextLines: 3,
  ignoreWhitespace: "none",
  wordDiff: true,
  detectMoves: true,
  laneWidth: 16,
  coloredLanes: false,
  pullMode: "ffOnly",
  gitPath: "git",
  terminal: "system",
  logLevel: "info",
  avatars: "gravatar",
  autoUpdate: false,
  confirmExit: true,
  backgroundFetchMinutes: 0,
};

/** Read once at startup, so changing them needs a restart to take effect. */
const RESTART_REQUIRED: readonly (keyof Settings)[] = ["logLevel", "gitPath"];

const ENUMS: Partial<Record<keyof Settings, readonly string[]>> = {
  theme: THEMES.map(([id]) => id),
  dateFormat: ["smart", "relative", "both"],
  algorithm: ["histogram", "myers"],
  ignoreWhitespace: ["none", "trailing", "all"],
  pullMode: ["ffOnly", "merge"],
  logLevel: ["error", "warn", "info", "debug", "trace"],
  avatars: ["ask", "gravatar", "off"],
};

const RANGES: Partial<Record<keyof Settings, [number, number]>> = {
  contextLines: [0, 50],
  laneWidth: [8, 40],
  backgroundFetchMinutes: [0, 1440],
};

export function needsRestart(key: keyof Settings): boolean {
  return RESTART_REQUIRED.includes(key);
}

/** Every stored value is re-checked: one bad key must not leave an unusable window. */
export function merge(stored: Partial<Settings> | null | undefined): Settings {
  const merged = { ...DEFAULT_SETTINGS };
  if (typeof stored !== "object" || stored === null) return merged;

  for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]) {
    const value = (stored as Record<string, unknown>)[key];
    const fallback = DEFAULT_SETTINGS[key];
    if (value === undefined || typeof value !== typeof fallback) continue;

    const options = ENUMS[key];
    if (options && !options.includes(value as string)) continue;

    const range = RANGES[key];
    if (range && typeof value === "number") {
      (merged[key] as number) = Math.min(Math.max(value, range[0]), range[1]);
      continue;
    }
    (merged[key] as unknown) = value;
  }
  return merged;
}
