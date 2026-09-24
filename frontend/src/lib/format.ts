import type { Branch, FileDiff, Head, LineEnding, StashEntry, Tag, WorktreeEntry } from "./ipc";

const SHORT_OID = 7;

export function headLabel(head: Head | null | undefined): string {
  if (!head) return "—";
  switch (head.kind) {
    case "branch":
      return head.name;
    case "detached":
      return `detached at ${head.oid.slice(0, SHORT_OID)}`;
    case "unborn":
      return `${head.name} (unborn)`;
  }
}

/** Order comes from the backend; sorting in both places would be two sources of truth. */
export function splitBranches(branches: Branch[]): { local: Branch[]; remote: Branch[] } {
  return {
    local: branches.filter((b) => b.kind === "local"),
    remote: branches.filter((b) => b.kind === "remote"),
  };
}

export function shortOid(oid: string): string {
  return oid.slice(0, SHORT_OID);
}

export type RefKind = "head" | "local" | "remote" | "tag" | "stash";

export interface RefLabel {
  text: string;
  kind: RefKind;
  /** Remotes whose branch of the same name is on this commit too, upstream first (#49). */
  remotes?: string[];
  /** The branch name drawn after `remotes` and `=`. */
  name?: string;
  /** The tooltip, when it says more than `text`. */
  title?: string;
  /** Another worktree has this branch checked out (#25). */
  worktree?: WorktreeMark;
}

export interface WorktreeMark {
  path: string;
  state: "clean" | "modified" | "missing";
}

function worktreeMark(entry: WorktreeEntry): WorktreeMark {
  return { path: entry.path, state: entry.missing ? "missing" : entry.dirty ? "modified" : "clean" };
}

/** The row truncates from the right, so order here is priority order. */
const REF_ORDER: Record<RefKind, number> = { head: 0, local: 1, remote: 2, tag: 3, stash: 4 };

/** What the graph labels besides branches and tags. */
export interface RefExtras {
  stashes?: readonly StashEntry[];
  worktrees?: readonly WorktreeEntry[];
}

const withoutRemote = (name: string) => name.slice(name.indexOf("/") + 1);
const remoteOf = (name: string) => name.slice(0, Math.max(name.indexOf("/"), 0));

/** Remote branches drawn inside the local label: its upstream, when it names the same
    branch on the same commit, and that branch on every other remote at that commit. */
function twins(local: Branch, remotes: ReadonlyMap<string, Branch>): Branch[] {
  const upstream = local.upstream === null ? undefined : remotes.get(local.upstream);
  if (!upstream || upstream.oid !== local.oid || withoutRemote(upstream.name) !== local.name) return [];
  const others = [...remotes.values()]
    .filter((r) => r !== upstream && r.oid === local.oid && withoutRemote(r.name) === local.name)
    .sort((a, b) => a.name.localeCompare(b.name));
  return [upstream, ...others];
}

export function refLabels(
  branches: Branch[],
  tags: Tag[],
  head: Head | null | undefined,
  extras: RefExtras = {},
): Map<string, RefLabel[]> {
  const headBranch = head?.kind === "branch" ? head.name : null;
  const byOid = new Map<string, RefLabel[]>();

  const add = (oid: string, label: RefLabel) => {
    const existing = byOid.get(oid);
    if (existing) existing.push(label);
    else byOid.set(oid, [label]);
  };

  const remotes = new Map(branches.filter((b) => b.kind === "remote").map((b) => [b.name, b]));
  const held = new Map(
    (extras.worktrees ?? [])
      .filter((entry) => entry.branch !== null && !entry.isCurrent)
      .map((entry) => [entry.branch as string, worktreeMark(entry)]),
  );
  const joined = new Set<string>();
  for (const branch of branches) {
    if (branch.kind !== "local") continue;
    const kind: RefKind = branch.name === headBranch ? "head" : "local";
    const found = twins(branch, remotes);
    for (const twin of found) joined.add(twin.name);
    const names = found.map((twin) => remoteOf(twin.name));
    const label: RefLabel =
      found.length === 0
        ? { text: branch.name, kind }
        : { text: `${names.join(",")}=${branch.name}`, kind, remotes: names, name: branch.name };
    const lines = found.length === 0 ? [] : [branch.name, ...found.map((twin) => twin.name)];
    const worktree = held.get(branch.name);
    if (worktree) {
      label.worktree = worktree;
      if (lines.length === 0) lines.push(branch.name);
      lines.push(`Checked out in worktree ${worktree.path} (${worktree.state})`);
    }
    if (lines.length > 0) label.title = lines.join("\n");
    add(branch.oid, label);
  }
  for (const branch of remotes.values()) {
    if (!joined.has(branch.name)) add(branch.oid, { text: branch.name, kind: "remote" });
  }
  for (const tag of tags) {
    add(tag.oid, { text: tag.name, kind: "tag" });
  }
  for (const stash of extras.stashes ?? []) {
    const text = `stash@{${stash.index}}`;
    add(stash.oid, { text, kind: "stash", title: `${text}\n${stash.message}` });
  }

  for (const labels of byOid.values()) {
    labels.sort((a, b) => REF_ORDER[a.kind] - REF_ORDER[b.kind]);
  }
  return byOid;
}

const UNITS: [seconds: number, unit: Intl.RelativeTimeFormatUnit][] = [
  [31_536_000, "year"],
  [2_592_000, "month"],
  [86_400, "day"],
  [3600, "hour"],
  [60, "minute"],
];

/** Pinned to English, like every other string in the UI: the default follows the system
    locale and would leave half the window in another language. `always`, not `auto`:
    "yesterday" belongs to the smart format, this one always spells the elapsed time. */
const RELATIVE = new Intl.RelativeTimeFormat("en", { numeric: "always" });

/** The offset is ignored: an elapsed time is the same number in every timezone. */
export function relativeDate(timestamp: number, _offsetMinutes: number, now: number): string {
  const elapsed = now - timestamp;
  for (const [seconds, unit] of UNITS) {
    const count = Math.floor(elapsed / seconds);
    if (count >= 1) return RELATIVE.format(-count, unit);
  }
  return "just now";
}

/** A commit with ten refs must not stretch the row; the rest go into a tooltip (T4.6). */
export function capsules(
  labels: readonly RefLabel[],
  room: number,
): { shown: RefLabel[]; hidden: RefLabel[] } {
  if (labels.length <= room) return { shown: [...labels], hidden: [] };
  return { shown: labels.slice(0, Math.max(room, 0)), hidden: labels.slice(Math.max(room, 0)) };
}

export function dateTooltip(timestamp: number, offsetMinutes: number): string {
  const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  const sign = offsetMinutes < 0 ? "-" : "+";
  const total = Math.abs(offsetMinutes);
  return (
    `${shifted.getUTCFullYear()}-${pad(shifted.getUTCMonth() + 1)}-${pad(shifted.getUTCDate())}` +
    ` ${pad(shifted.getUTCHours())}:${pad(shifted.getUTCMinutes())}:${pad(shifted.getUTCSeconds())}` +
    ` ${sign}${pad(Math.floor(total / 60))}:${pad(total % 60)}`
  );
}

export type DateMode = "smart" | "relative" | "both";

const WEEKDAYS = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
];

const DAY_SECONDS = 86_400;

function dayNumber(timestamp: number, offsetMinutes: number): number {
  return Math.floor((timestamp + offsetMinutes * 60) / DAY_SECONDS);
}

/** `DD-MM-YY`: the short form for anything the weekday can no longer place. */
export function shortDate(timestamp: number, offsetMinutes: number): string {
  const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(shifted.getUTCDate())}-${pad(shifted.getUTCMonth() + 1)}-${pad(
    shifted.getUTCFullYear() % 100,
  )}`;
}

/** Days are counted in the commit's own timezone, so the answer never depends on the
    machine reading it. Anything older than a week gets a date instead of a weekday. */
export function smartDate(timestamp: number, offsetMinutes: number, now: number): string {
  const elapsed = dayNumber(now, offsetMinutes) - dayNumber(timestamp, offsetMinutes);
  if (elapsed === 0) return "today";
  if (elapsed === 1) return "yesterday";
  if (elapsed > 1 && elapsed < 7) {
    const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
    return WEEKDAYS[shifted.getUTCDay()] ?? shortDate(timestamp, offsetMinutes);
  }
  return shortDate(timestamp, offsetMinutes);
}

export function displayDate(
  timestamp: number,
  offsetMinutes: number,
  now: number,
  mode: DateMode,
): string {
  const smart = smartDate(timestamp, offsetMinutes, now);
  const elapsed = relativeDate(timestamp, offsetMinutes, now);
  if (mode === "smart") return smart;
  if (mode === "relative") return elapsed;
  return `${smart} · ${elapsed}`;
}

const LINE_ENDINGS: Record<LineEnding, string | undefined> = {
  lf: "LF",
  crlf: "CRLF",
  cr: "CR",
  mixed: "Mixed",
  none: undefined,
};

/** What the status bar says about the file in the diff panel: nothing for a picture, a
    binary or no file at all, and the line ending of the side on disk now. */
export function fileFormat(diff: FileDiff | null): { encoding?: string; lineEnding?: string } {
  if (diff?.kind !== "text") return {};
  return {
    encoding: diff.lossyEncoding ? "Not UTF-8" : "UTF-8",
    lineEnding: LINE_ENDINGS[diff.eol.new] ?? LINE_ENDINGS[diff.eol.old] ?? "LF",
  };
}
