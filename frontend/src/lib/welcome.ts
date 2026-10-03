import type { FolderKind } from "$lib/ipc/clone";

/** The Welcome dialog (F-586): the pure rules; the store and the component only apply them. */

export interface StartupFacts {
  enabled: boolean;
  /** The saved session is still being reopened. */
  restoring: boolean;
  openCount: number;
  phase: "closed" | "opening" | "open" | "failed";
}

/** Only after Closed → Opening → Open | Failed has settled with nothing open: never before a
    session is restored, so it cannot flash over the repository about to appear. */
export function shouldShowAtStartup(facts: StartupFacts): boolean {
  return facts.enabled && !facts.restoring && facts.openCount === 0 && (facts.phase === "closed" || facts.phase === "failed");
}

export interface MruRow {
  path: string;
  name: string;
}

export function repoName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const last = trimmed.split(/[\\/]/).at(-1) ?? "";
  return last === "" || last.endsWith(":") ? path : last;
}

/** Windows paths ignore case and slash direction; a POSIX path keeps its case. */
export function pathKey(path: string): string {
  const normal = path.replace(/\\/g, "/").replace(/\/+$/, "");
  return /^([a-z]:|\/\/)/i.test(normal) ? normal.toLowerCase() : normal;
}

/** The recent list as it was kept (newest first), each repository once. The full path is
    always shown beside the name, so two `app` folders are told apart by it. */
export function mruRows(recent: readonly string[]): MruRow[] {
  const seen = new Set<string>();
  const rows: MruRow[] = [];
  for (const path of recent) {
    const key = pathKey(path);
    if (seen.has(key)) continue;
    seen.add(key);
    rows.push({ path, name: repoName(path) });
  }
  return rows;
}

/** `unknown`: the check ended without an answer (a timeout, an error); the row stays
    usable and opening it says what is wrong. */
export type Availability = "checking" | "available" | "missing" | "notRepository" | "unknown";

export function availabilityOf(kind: FolderKind | null): Availability {
  switch (kind) {
    case "repository":
      return "available";
    case "plain":
      return "notRepository";
    case "missing":
    case "file":
      return "missing";
    default:
      return "unknown";
  }
}

export function noteFor(availability: Availability): string | null {
  if (availability === "missing") return "not found";
  if (availability === "notRepository") return "not a repository";
  return null;
}

export function isUnavailable(availability: Availability | undefined): boolean {
  return availability === "missing" || availability === "notRepository";
}

export interface CheckOptions {
  timeoutMs: number;
  /** At most this many paths are asked at once; a dead share holds one slot, not all. */
  concurrency?: number;
  signal?: AbortSignal;
  onResult: (path: string, availability: Availability) => void;
}

/** Each path is reported as its own check ends. A path that does not answer in time is
    `unknown`; after `signal` aborts nothing more is reported. */
export async function checkAvailability(
  paths: readonly string[],
  probe: (path: string) => Promise<FolderKind>,
  { timeoutMs, concurrency = 4, signal, onResult }: CheckOptions,
): Promise<void> {
  const queue = [...paths];

  async function one(path: string): Promise<void> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<null>((resolve) => {
      timer = setTimeout(() => resolve(null), timeoutMs);
    });
    const kind = await Promise.race([probe(path).catch(() => null), timeout]);
    clearTimeout(timer);
    if (!signal?.aborted) onResult(path, availabilityOf(kind));
  }

  async function lane(): Promise<void> {
    for (let path = queue.shift(); path !== undefined; path = queue.shift()) {
      if (signal?.aborted) return;
      await one(path);
    }
  }

  await Promise.all(Array.from({ length: Math.min(concurrency, queue.length) }, lane));
}

/** Above this many rows a filter field stands over the list. */
export const FILTER_ABOVE = 8;

export function showsFilter(rowCount: number): boolean {
  return rowCount > FILTER_ABOVE;
}

/** Rows whose name or path holds the text, case-insensitive; an empty filter keeps all. */
export function filterRows(rows: readonly MruRow[], query: string): MruRow[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return [...rows];
  return rows.filter((row) => row.name.toLowerCase().includes(needle) || row.path.toLowerCase().includes(needle));
}

export type Step = "up" | "down" | "home" | "end";

/** The arrows walk the rows shown; nothing selected starts at the first (or, going up, the last). */
export function moveSelection(rows: readonly MruRow[], selected: string | null, step: Step): string | null {
  if (rows.length === 0) return null;
  const at = rows.findIndex((row) => row.path === selected);
  const last = rows.length - 1;
  const next =
    step === "home" ? 0
    : step === "end" ? last
    : at < 0 ? (step === "down" ? 0 : last)
    : Math.min(Math.max(at + (step === "down" ? 1 : -1), 0), last);
  return rows[next]?.path ?? null;
}

/** A filter that hides the selected row moves the selection to the first row it shows. */
export function keepSelection(rows: readonly MruRow[], selected: string | null): string | null {
  if (rows.some((row) => row.path === selected)) return selected;
  return rows[0]?.path ?? null;
}

/** Rows about to go: the selection moves to the next row that stays, else the one before. */
export function selectionAfterRemoval(
  rows: readonly MruRow[],
  selected: string | null,
  removed: ReadonlySet<string>,
): string | null {
  if (selected === null || !removed.has(selected)) return selected;
  const at = rows.findIndex((row) => row.path === selected);
  const after = rows.slice(at + 1).find((row) => !removed.has(row.path));
  const before = rows.slice(0, Math.max(at, 0)).reverse().find((row) => !removed.has(row.path));
  return (after ?? before)?.path ?? null;
}

/** The rows "Remove All Missing" takes: not found, or no longer a repository. */
export function missingPaths(rows: readonly MruRow[], availability: ReadonlyMap<string, Availability>): string[] {
  return rows.filter((row) => isUnavailable(availability.get(row.path))).map((row) => row.path);
}

/** "Remove All Missing" asks first, naming how many go; "Remove from List" for one row does not. */
export function forgetMissingQuestion(paths: readonly string[]) {
  const what = paths.length === 1 ? "1 missing repository" : `${paths.length} missing repositories`;
  return { title: "Remove All Missing", message: `Remove ${what} from the list?`, confirm: "Remove", items: paths };
}

/** Nothing is asked when nothing is missing; the paths go only on a yes. */
export async function forgetMissing(
  paths: readonly string[],
  ask: (question: ReturnType<typeof forgetMissingQuestion>) => Promise<boolean>,
  forget: (paths: readonly string[]) => void,
): Promise<void> {
  if (paths.length > 0 && (await ask(forgetMissingQuestion(paths)))) forget(paths);
}

/** What Open (or Enter, or a double click) opens: the selected row, unless it is gone. */
export function openTarget(
  rows: readonly MruRow[],
  selected: string | null,
  availability: ReadonlyMap<string, Availability>,
): string | null {
  const row = rows.find((entry) => entry.path === selected);
  return row && !isUnavailable(availability.get(row.path)) ? row.path : null;
}

export type WelcomeAction = { kind: "folder" } | { kind: "clone" } | { kind: "open"; path: string };

/** What to do with the folder Open or Create… picked. */
export function folderPlan(kind: FolderKind): "open" | "init" | "gone" {
  if (kind === "repository") return "open";
  return kind === "plain" ? "init" : "gone";
}

/** The ids of the recent list's context menu. */
export const WELCOME_FORGET = "welcome-forget";
export const WELCOME_FORGET_MISSING = "welcome-forget-missing";
