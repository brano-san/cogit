import type { Branch, FileDiff, Head, LineEnding, StashEntry, Tag, WorktreeEntry } from "./ipc";
import { splitUpstream } from "./push-to";

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

/** A revision that may be a full id with `^` or `~n` after it: the id is shortened, the rest
    kept, so `<oid>^` still reads as the parent. */
export function shortRev(rev: string): string {
  const id = /^[0-9a-f]{40,64}/.exec(rev);
  return id ? shortOid(id[0]) + rev.slice(id[0].length) : rev;
}

export type RefKind = "head" | "local" | "remote" | "tag" | "stash";

export interface RefLabel {
  text: string;
  kind: RefKind;
  ref?: string;
  /** The upstream on the same commit, joined into this label (#49): `origin/master`. */
  upstream?: string;
  /** What is drawn before `=`: the remote alone when the upstream has the local branch's
      name (`origin=master`), else the upstream in full (`origin/master=bugfix/x`). */
  prefix?: string;
  /** The branch name drawn after `prefix` and `=`. */
  name?: string;
  /** The tooltip, when it says more than `text`. */
  title?: string;
  /** Another worktree has this branch checked out (#25). */
  worktree?: WorktreeMark;
  /** A local branch apart from its upstream: `+2 -1`, a block right before the label. */
  divergence?: string;
  divergenceTitle?: string;
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
  /** The configured remotes, to tell `team/eu` from a branch `eu/x` of `team`. Without
      them the remote is what comes before the first `/`. */
  remotes?: readonly string[];
}

/** The upstream drawn inside the local label: the remote-tracking branch configured as
    upstream (a local or a gone upstream is not in `remotes`), on the same commit. */
function joinedUpstream(local: Branch, remotes: ReadonlyMap<string, Branch>): Branch | undefined {
  const upstream = local.upstream === null ? undefined : remotes.get(local.upstream);
  return upstream?.oid === local.oid ? upstream : undefined;
}

function prefixOf(upstream: string, local: string, configured: readonly string[] | undefined): string {
  const cut = upstream.indexOf("/");
  const split = configured
    ? splitUpstream(upstream, configured)
    : cut < 0
      ? null
      : { remote: upstream.slice(0, cut), branch: upstream.slice(cut + 1) };
  return split?.branch === local ? split.remote : upstream;
}

/** The two refs of a joined label, each as a label of its own: what the menu, the tooltip
    and the double click of that part act on. `remote` is null for any other label. */
export function labelParts(label: RefLabel): { remote: RefLabel | null; local: RefLabel } {
  if (label.upstream === undefined) return { remote: null, local: label };
  const { upstream: _upstream, prefix: _prefix, ...local } = label;
  return {
    remote: { text: label.upstream, kind: "remote", ref: label.upstream },
    local: { ...local, text: label.name ?? label.text },
  };
}

export function shortRef(name: string): string {
  return /(^|\/)checkpoints\//.test(name) ? "checkpoint" : name;
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
    const upstream = joinedUpstream(branch, remotes);
    if (upstream) joined.add(upstream.name);
    const prefix = upstream && prefixOf(upstream.name, branch.name, extras.remotes);
    const label: RefLabel = !upstream
      ? { text: shortRef(branch.name), kind, ref: branch.name }
      : { text: `${prefix}=${branch.name}`, kind, ref: branch.name, upstream: upstream.name, prefix, name: branch.name };
    const lines = upstream ? [branch.name, upstream.name] : [];
    const worktree = held.get(branch.name);
    if (worktree) {
      label.worktree = worktree;
      if (lines.length === 0) lines.push(branch.name);
      lines.push(`Checked out in worktree ${worktree.path} (${worktree.state})`);
    }
    if (lines.length === 0 && label.text !== branch.name) lines.push(branch.name);
    if (lines.length > 0) label.title = lines.join("\n");
    const ahead = branch.ahead;
    if (branch.upstream !== null && ahead + branch.behind > 0) {
      const counts: [string, string][] = [];
      if (ahead > 0) counts.push([`+${ahead}`, `${ahead} ahead`]);
      if (branch.behind > 0) counts.push([`-${branch.behind}`, `${branch.behind} behind`]);
      label.divergence = counts.map(([short]) => short).join(" ");
      label.divergenceTitle = `${branch.upstream}: ${counts.map(([, long]) => long).join(", ")}`;
    }
    add(branch.oid, label);
  }
  for (const branch of remotes.values()) {
    if (joined.has(branch.name)) continue;
    const text = shortRef(branch.name);
    add(
      branch.oid,
      text === branch.name
        ? { text, kind: "remote", ref: branch.name }
        : { text, kind: "remote", ref: branch.name, title: branch.name },
    );
  }
  for (const tag of tags) {
    add(tag.oid, { text: tag.name, kind: "tag", ref: tag.name });
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

const UNITS: [seconds: number, unit: string][] = [
  [31_536_000, "year"],
  [2_592_000, "month"],
  [86_400, "day"],
  [3600, "hour"],
  [60, "minute"],
];

/** Spelled out here, not by `Intl`: the text must not depend on the locale or the ICU data
    the webview has (WebKitGTK under `LANG=C`), and English is all the UI speaks. Always the
    elapsed time: "Yesterday" belongs to the smart format. The offset is ignored: an elapsed
    time is the same number in every timezone. */
export function relativeDate(timestamp: number, _offsetMinutes: number, now: number): string {
  const elapsed = now - timestamp;
  for (const [seconds, unit] of UNITS) {
    const count = Math.floor(elapsed / seconds);
    if (count >= 1) return `${count} ${unit}${count === 1 ? "" : "s"} ago`;
  }
  return "Just now";
}

/** A branch and a tag may share a name on one commit, so the text alone repeats. */
export function refLabelKey(label: RefLabel): string {
  return `${label.kind}:${label.text}`;
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

/** `DD-MM-YY` (`DD-MM-YYYY` with `fullYear`): the form for anything the weekday can no
    longer place. */
export function shortDate(timestamp: number, offsetMinutes: number, fullYear = false): string {
  const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  const year = shifted.getUTCFullYear();
  return `${pad(shifted.getUTCDate())}-${pad(shifted.getUTCMonth() + 1)}-${
    fullYear ? year : pad(year % 100)
  }`;
}

/** Days are counted in the commit's own timezone, so the answer never depends on the
    machine reading it. Anything older than a week gets a date instead of a weekday. */
export function smartDate(
  timestamp: number,
  offsetMinutes: number,
  now: number,
  fullYear = false,
): string {
  const elapsed = dayNumber(now, offsetMinutes) - dayNumber(timestamp, offsetMinutes);
  if (elapsed === 0) return "Today";
  if (elapsed === 1) return "Yesterday";
  if (elapsed > 1 && elapsed < 7) {
    const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
    return WEEKDAYS[shifted.getUTCDay()] ?? shortDate(timestamp, offsetMinutes, fullYear);
  }
  return shortDate(timestamp, offsetMinutes, fullYear);
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
