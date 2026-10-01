import type { FolderKind } from "$lib/ipc/clone";

/** The Welcome dialog (F-586): the pure rules; the store and the component only apply them. */

export type WelcomeOption = 1 | 2 | 3;

/** `row` is the selected recent repository; it only counts while `option` is 3. */
export interface WelcomeSelection {
  option: WelcomeOption;
  row: number | null;
}

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

export function defaultSelection(rowCount: number): WelcomeSelection {
  return rowCount > 0 ? { option: 3, row: 0 } : { option: 1, row: null };
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

/** Option 3 reopens a recent repository: with none it is disabled and nothing can select it. */
export function isOptionDisabled(option: WelcomeOption, rowCount: number): boolean {
  return option === 3 && rowCount === 0;
}

/** One line of stops: option 1, option 2, then each recent repository (option 3 is the
    list; with an empty list it is no stop at all). */
function stops(rowCount: number): number {
  return 2 + rowCount;
}

export function moveSelection(sel: WelcomeSelection, dir: "up" | "down", rowCount: number): WelcomeSelection {
  const at = sel.option === 1 ? 0 : sel.option === 2 ? 1 : 2 + (sel.row ?? 0);
  const next = Math.min(Math.max(at + (dir === "down" ? 1 : -1), 0), stops(rowCount) - 1);
  if (next === 0) return { option: 1, row: sel.row };
  if (next === 1) return { option: 2, row: sel.row };
  return { option: 3, row: next - 2 };
}

export function selectOption(sel: WelcomeSelection, option: WelcomeOption, rowCount: number): WelcomeSelection {
  if (isOptionDisabled(option, rowCount)) return sel;
  if (option !== 3) return { option, row: sel.row };
  return { option, row: rowCount > 0 ? Math.min(sel.row ?? 0, rowCount - 1) : null };
}

/** After the list changed under the selection (a row removed). */
export function clampSelection(sel: WelcomeSelection, rowCount: number): WelcomeSelection {
  if (rowCount === 0) return { option: sel.option === 3 ? 1 : sel.option, row: null };
  return { option: sel.option, row: Math.min(sel.row ?? 0, rowCount - 1) };
}

export type WelcomeAction = { kind: "folder" } | { kind: "clone" } | { kind: "open"; path: string };

export function okAction(sel: WelcomeSelection, rows: readonly MruRow[]): WelcomeAction | null {
  if (sel.option === 1) return { kind: "folder" };
  if (sel.option === 2) return { kind: "clone" };
  const row = sel.row === null ? undefined : rows[sel.row];
  return row ? { kind: "open", path: row.path } : null;
}

/** What to do with the folder option 1 picked. */
export function folderPlan(kind: FolderKind): "open" | "init" | "gone" {
  if (kind === "repository") return "open";
  return kind === "plain" ? "init" : "gone";
}

/** The id of the one item of the recent list's context menu. */
export const WELCOME_FORGET = "welcome-forget";
