import type { DateMode } from "$lib/format";
import type { Algorithm, Whitespace } from "$lib/ipc";
import { LANE_WIDTH } from "$lib/graph-geometry";
import { DEFAULT_FILTER_FIELDS, knownFields, type FilterField } from "$lib/filter-fields";
import { knownPatterns } from "$lib/filter-patterns";
import { GRAPH_COLORINGS, migratedColoring, type GraphColoring } from "$lib/graph-coloring";

/** Lightest first; the grey ones sit between the extremes (#24). */
export const THEMES = [
  ["light", "Light"],
  ["lightGrey", "Light gray"],
  ["darkGrey", "Dark gray"],
  ["dark", "Dark"],
] as const;

export type DiffLayout = "aligned" | "compact";

export type Theme = (typeof THEMES)[number][0];

/** The commit list's optional columns; the subject and the ref capsules always show. */
export const GRAPH_COLUMNS = ["hash", "author", "avatar", "time"] as const;
export type GraphColumn = (typeof GRAPH_COLUMNS)[number];
export type GraphTimeFormat = "relative" | "date" | "dateTime";
export type GraphDensity = "compact" | "normal" | "comfortable";

/** Who draws the menu bar, the drop-downs, the context menus: the page (`on`), the platform
    (`off`), or the platform's own choice (`auto`: the page on Linux, the platform on Windows
    and macOS). A window with no decorations of its own gets the page's titlebar either way. */
export type WebMenus = "auto" | "on" | "off";

export interface Settings {
  theme: Theme;
  dateFormat: DateMode;
  algorithm: Algorithm;
  contextLines: number;
  ignoreWhitespace: Whitespace;
  wordDiff: boolean;
  detectMoves: boolean;
  /** Side by side: the left code column's share of the width both get (R-535). */
  diffSplit: number;
  /** Side by side: `aligned` keeps both sides on the same rows (filler where one has no lines),
      `compact` is each side's own rows per side with curved connectors (08 §12.2). */
  diffLayout: DiffLayout;
  laneWidth: number;
  pullMode: "ffOnly" | "merge";
  gitPath: string;
  terminal: string;
  /** The external merge tool the Conflict Solver opens: a program, and its arguments with
      {base} {ours} {theirs} {result}. Empty: git's `merge.tool`. */
  mergeExternalTool: string;
  mergeExternalToolArgs: string;
  logLevel: "error" | "warn" | "info" | "debug" | "trace";
  /** On by default: a face per row is what the commit list is read by, and the request
      carries a hash rather than the address (doc/12-risks.md, R-145). `ask` is what a
      settings file written before that default said. */
  avatars: "ask" | "gravatar" | "off";
  /** Menus and context menus drawn by the page; the titlebar follows the window's decorations. Read at start. */
  uiWebMenus: WebMenus;
  /** Off until ticked: nothing reaches the network unasked. Help ▸ Check for Updates…
      works either way. */
  autoUpdate: boolean;
  /** The Welcome dialog at startup when no repository is open; nothing else opens it. */
  startupShowWelcome: boolean;
  /** Off through the Exit dialog's "Don't show again"; back on through Preferences → Behavior →
      Don't show again (R-151). */
  confirmExit: boolean;
  /** The Checkout dialog for a local branch; its "Don't show again" turns it off (item 40). */
  confirmLocalCheckout: boolean;
  /** Taskbar button: progress of long operations, the overlay badge for errors, warnings and unviewed events. */
  notificationsTaskbar: boolean;
  /** Blinking the taskbar button while the window is in the background; needs `notificationsTaskbar`. */
  notificationsTaskbarFlash: boolean;
  /** A system notification when an operation of more than a few seconds ends in the background. */
  notificationsSystem: boolean;
  /** Needs `notificationsSystem`. */
  notificationsSystemSuccess: boolean;
  /** Failures and conflicts; needs `notificationsSystem`. */
  notificationsSystemFailure: boolean;
  /** Branches ▸ Other Refs also lists ORIG_HEAD, MERGE_HEAD and the other pseudo-refs. */
  refsShowPseudoRefs: boolean;
  /** The visible columns, left to right; a hidden one is simply absent. */
  graphColumns: GraphColumn[];
  /** The graph's time column only; `dateFormat` stays for Blame and commit details (R-370). */
  graphTimeFormat: GraphTimeFormat;
  /** Which one-time migrations the file has been through (`SETTINGS_VERSION`); not a preference. */
  settingsVersion: number;
  graphDensity: GraphDensity;
  graphStripes: boolean;
  /** A note icon on commits that have a git note; hover shows the note. */
  graphShowNotes: boolean;
  graphAvatarsChangedOnly: boolean;
  /** A link longer than this many rows is drawn as two stubs; 0 draws every link whole. */
  graphLongLinkRows: number;
  graphHighlightChecked: boolean;
  /** Graph colorings; `varying` is a color per lane, `branch` brings the selected
      commit's branch forward (R-574). */
  graphColoring: GraphColoring;
  graphFirstParent: boolean;
  graphAncestry: boolean;
  graphCollapseMerged: boolean;
  /** Show Only Selected Branches and Tags: labels only for refs ticked in Branches (F-561). */
  graphSelectedRefsOnly: boolean;
  /** Include Tracked Remote Branches: a ticked branch walks its upstream too (F-561). */
  graphIncludeTracked: boolean;
  /** Show Graph While Filtering: lines between a filter's matches, not a flat list (F-561). */
  graphWhileFiltering: boolean;
  /** Show Working Tree Permanently: off, no Working Tree row while it is clean (F-561). */
  graphWorkingTreeAlways: boolean;
  /** Where the graph filter looks for its text: the switches under the field (F-560). */
  graphFilterFields: FilterField[];
  /** Filter texts kept by Remember Pattern, newest first (F-563). */
  graphFilterPatterns: string[];

  /** Minutes between asking the remote of every listed repository what it has (R-354);
      `0` is off, which is the default: nothing reaches the network unasked. */
  backgroundFetchMinutes: number;
}

/** 1: a stored graph `date` became `dateTime` (R-370). */
export const SETTINGS_VERSION = 1;
/** Kept in the file but never shown in Preferences. */
export const INTERNAL_SETTINGS: readonly (keyof Settings)[] = ["settingsVersion"];

export const DEFAULT_SETTINGS: Settings = {
  theme: "dark",
  dateFormat: "smart",
  algorithm: "histogram",
  contextLines: 3,
  ignoreWhitespace: "none",
  wordDiff: true,
  detectMoves: true,
  diffSplit: 0.5,
  diffLayout: "aligned",
  laneWidth: LANE_WIDTH.default,
  pullMode: "ffOnly",
  gitPath: "git",
  terminal: "system",
  mergeExternalTool: "",
  mergeExternalToolArgs: "",
  logLevel: "info",
  avatars: "gravatar",
  uiWebMenus: "auto",
  autoUpdate: false,
  startupShowWelcome: true,
  confirmExit: true,
  confirmLocalCheckout: true,
  notificationsTaskbar: true,
  notificationsTaskbarFlash: true,
  notificationsSystem: true,
  notificationsSystemSuccess: true,
  notificationsSystemFailure: true,
  refsShowPseudoRefs: false,
  graphColumns: ["author", "avatar", "time", "hash"],
  graphTimeFormat: "dateTime",
  settingsVersion: SETTINGS_VERSION,
  graphDensity: "normal",
  graphStripes: true,
  graphShowNotes: true,
  graphAvatarsChangedOnly: false,
  graphLongLinkRows: 40,
  graphHighlightChecked: true,
  graphColoring: "default",
  graphFirstParent: false,
  graphAncestry: false,
  graphCollapseMerged: false,
  graphSelectedRefsOnly: false,
  graphIncludeTracked: false,
  graphWhileFiltering: true,
  graphWorkingTreeAlways: true,
  graphFilterFields: [...DEFAULT_FILTER_FIELDS],
  graphFilterPatterns: [],

  backgroundFetchMinutes: 0,
};

/** Read once at startup, so changing them needs a restart to take effect. */
const RESTART_REQUIRED: readonly (keyof Settings)[] = ["logLevel", "uiWebMenus"];

const ENUMS: Partial<Record<keyof Settings, readonly string[]>> = {
  theme: THEMES.map(([id]) => id),
  dateFormat: ["smart", "relative", "both"],
  algorithm: ["histogram", "myers"],
  ignoreWhitespace: ["none", "trailing", "all"],
  diffLayout: ["aligned", "compact"],
  pullMode: ["ffOnly", "merge"],
  logLevel: ["error", "warn", "info", "debug", "trace"],
  avatars: ["ask", "gravatar", "off"],
  uiWebMenus: ["auto", "on", "off"],
  graphTimeFormat: ["relative", "date", "dateTime"],
  graphDensity: ["compact", "normal", "comfortable"],
  graphColoring: GRAPH_COLORINGS,
};

const RANGES: Partial<Record<keyof Settings, [number, number]>> = {
  contextLines: [0, 50],
  diffSplit: [0.2, 0.8],
  laneWidth: [LANE_WIDTH.min, LANE_WIDTH.max],
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
  // The whole file is written on every save, so an old `date` was most likely never chosen:
  // `dateTime` became the default for everyone, once (R-370).
  const version = typeof stored.settingsVersion === "number" ? stored.settingsVersion : 0;
  if (stored.graphTimeFormat === "date" && version < 1) return "dateTime";
  if (stored.graphTimeFormat !== undefined) return undefined;
  return stored.dateFormat === "relative" ? "relative" : undefined;
}

export function needsRestart(key: keyof Settings): boolean {
  return RESTART_REQUIRED.includes(key);
}

/** Every stored value is re-checked: one bad key must not leave an unusable window. */
export function merge(stored: Partial<Settings> | null | undefined): Settings {
  const merged = {
    ...DEFAULT_SETTINGS,
    graphColumns: [...DEFAULT_SETTINGS.graphColumns],
    graphFilterFields: [...DEFAULT_SETTINGS.graphFilterFields],
    graphFilterPatterns: [...DEFAULT_SETTINGS.graphFilterPatterns],
  };
  if (typeof stored !== "object" || stored === null) return merged;
  const record = stored as Record<string, unknown>;

  for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]) {
    const value = record[key];
    const fallback = DEFAULT_SETTINGS[key];
    if (key === "graphColumns") {
      merged.graphColumns = knownColumns(value) ?? merged.graphColumns;
      continue;
    }
    if (key === "graphFilterFields") {
      merged.graphFilterFields = knownFields(value) ?? merged.graphFilterFields;
      continue;
    }
    if (key === "graphFilterPatterns") {
      merged.graphFilterPatterns = knownPatterns(value) ?? merged.graphFilterPatterns;
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
  merged.settingsVersion = SETTINGS_VERSION;
  merged.graphColoring = migratedColoring(record) ?? merged.graphColoring;
  return merged;
}
