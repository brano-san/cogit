import type { DateMode } from "$lib/format";
import type { Algorithm, Whitespace } from "$lib/ipc";

export interface Settings {
  theme: "dark" | "light";
  dateFormat: DateMode;
  algorithm: Algorithm;
  contextLines: number;
  ignoreWhitespace: Whitespace;
  wordDiff: boolean;
  detectMoves: boolean;
  laneWidth: number;
  pullMode: "ffOnly" | "merge";
  gitPath: string;
  terminal: string;
  logLevel: "error" | "warn" | "info" | "debug" | "trace";
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "dark",
  dateFormat: "smart",
  algorithm: "histogram",
  contextLines: 3,
  ignoreWhitespace: "none",
  wordDiff: true,
  detectMoves: true,
  laneWidth: 14,
  pullMode: "ffOnly",
  gitPath: "git",
  terminal: "system",
  logLevel: "info",
};

/** Read once at startup, so changing them needs a restart to take effect. */
const RESTART_REQUIRED: readonly (keyof Settings)[] = ["logLevel", "gitPath"];

const ENUMS: Partial<Record<keyof Settings, readonly string[]>> = {
  theme: ["dark", "light"],
  dateFormat: ["smart", "relative", "both"],
  algorithm: ["histogram", "myers"],
  ignoreWhitespace: ["none", "trailing", "all"],
  pullMode: ["ffOnly", "merge"],
  logLevel: ["error", "warn", "info", "debug", "trace"],
};

const RANGES: Partial<Record<keyof Settings, [number, number]>> = {
  contextLines: [0, 50],
  laneWidth: [8, 40],
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
