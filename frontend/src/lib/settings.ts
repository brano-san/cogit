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

/** The commit list's optional columns; the subject and the ref capsules always show. */
export const GRAPH_COLUMNS = ["hash", "author", "avatar", "time"] as const;
export type GraphColumn = (typeof GRAPH_COLUMNS)[number];
export type GraphTimeFormat = "relative" | "date" | "dateTime";
export type GraphDensity = "compact" | "normal" | "comfortable";

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
  /** The visible columns, left to right; a hidden one is simply absent. */
  graphColumns: GraphColumn[];
  /** The graph's time column only; `dateFormat` stays for Blame and commit details (R-370). */
  graphTimeFormat: GraphTimeFormat;
  graphDensity: GraphDensity;
  graphStripes: boolean;
  /** A link longer than this many rows is drawn as two stubs; 0 draws every link whole. */
  graphLongLinkRows: number;
  graphHighlightChecked: boolean;
  graphFirstParent: boolean;
  graphBranchOfCommit: boolean;
  graphAncestry: boolean;
  graphCollapseMerged: boolean;

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
  graphColumns: ["author", "avatar", "time", "hash"],
  graphTimeFormat: "date",
  graphDensity: "normal",
  graphStripes: true,
  graphLongLinkRows: 40,
  graphHighlightChecked: true,
  graphFirstParent: false,
  graphBranchOfCommit: false,
  graphAncestry: false,
  graphCollapseMerged: false,

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
  graphTimeFormat: ["relative", "date", "dateTime"],
  graphDensity: ["compact", "normal", "comfortable"],
};

const RANGES: Partial<Record<keyof Settings, [number, number]>> = {
  contextLines: [0, 50],
  laneWidth: [8, 40],
  backgroundFetchMinutes: [0, 1440],
};

export const LONG_LINK_ROWS_MAX = 1000;

/** Unlike `RANGES`, a value outside these is not clamped: it falls back to the default. */
const CHECKS: Partial<Record<keyof Settings, (value: unknown) => boolean>> = {
  graphLongLinkRows: (value) =>
    Number.isInteger(value) && (value as number) >= 0 && (value as number) <= LONG_LINK_ROWS_MAX,
};

/** Known columns only, each once: a column a newer version added does not wipe the rest. */
function knownColumns(value: unknown): GraphColumn[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const known = value.filter((entry): entry is GraphColumn =>
    (GRAPH_COLUMNS as readonly unknown[]).includes(entry),
  );
  return [...new Set(known)];
}

/** Before the graph had its own time format it followed `dateFormat`: a file written then
    keeps the graph as it looked. */
function migratedTimeFormat(stored: Record<string, unknown>): GraphTimeFormat | undefined {
  if (stored.graphTimeFormat !== undefined) return undefined;
  return stored.dateFormat === "relative" ? "relative" : undefined;
}

export function needsRestart(key: keyof Settings): boolean {
  return RESTART_REQUIRED.includes(key);
}

/** Every stored value is re-checked: one bad key must not leave an unusable window. */
export function merge(stored: Partial<Settings> | null | undefined): Settings {
  const merged = { ...DEFAULT_SETTINGS, graphColumns: [...DEFAULT_SETTINGS.graphColumns] };
  if (typeof stored !== "object" || stored === null) return merged;
  const record = stored as Record<string, unknown>;

  for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]) {
    const value = record[key];
    const fallback = DEFAULT_SETTINGS[key];
    if (key === "graphColumns") {
      merged.graphColumns = knownColumns(value) ?? merged.graphColumns;
      continue;
    }
    if (value === undefined || typeof value !== typeof fallback) continue;

    const options = ENUMS[key];
    if (options && !options.includes(value as string)) continue;

    const check = CHECKS[key];
    if (check && !check(value)) continue;

    const range = RANGES[key];
    if (range && typeof value === "number") {
      (merged[key] as number) = Math.min(Math.max(value, range[0]), range[1]);
      continue;
    }
    (merged[key] as unknown) = value;
  }
  merged.graphTimeFormat = migratedTimeFormat(record) ?? merged.graphTimeFormat;
  return merged;
}
