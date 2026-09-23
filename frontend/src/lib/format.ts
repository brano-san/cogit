import type { Branch, Head, Tag } from "./ipc";

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

export type RefKind = "head" | "local" | "remote" | "tag";

export interface RefLabel {
  text: string;
  kind: RefKind;
  /** Remotes whose branch of the same name is on this commit too, upstream first (#49). */
  remotes?: string[];
  /** The branch name drawn after `remotes` and `=`. */
  name?: string;
  /** The tooltip, when it says more than `text`. */
  title?: string;
}

/** The row truncates from the right, so order here is priority order. */
const REF_ORDER: Record<RefKind, number> = { head: 0, local: 1, remote: 2, tag: 3 };

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
): Map<string, RefLabel[]> {
  const headBranch = head?.kind === "branch" ? head.name : null;
  const byOid = new Map<string, RefLabel[]>();

  const add = (oid: string, label: RefLabel) => {
    const existing = byOid.get(oid);
    if (existing) existing.push(label);
    else byOid.set(oid, [label]);
  };

  const remotes = new Map(branches.filter((b) => b.kind === "remote").map((b) => [b.name, b]));
  const joined = new Set<string>();
  for (const branch of branches) {
    if (branch.kind !== "local") continue;
    const kind: RefKind = branch.name === headBranch ? "head" : "local";
    const found = twins(branch, remotes);
    if (found.length === 0) {
      add(branch.oid, { text: branch.name, kind });
      continue;
    }
    for (const twin of found) joined.add(twin.name);
    const names = found.map((twin) => remoteOf(twin.name));
    add(branch.oid, {
      text: `${names.join(",")}=${branch.name}`,
      kind,
      remotes: names,
      name: branch.name,
      title: [branch.name, ...found.map((twin) => twin.name)].join("\n"),
    });
  }
  for (const branch of remotes.values()) {
    if (!joined.has(branch.name)) add(branch.oid, { text: branch.name, kind: "remote" });
  }
  for (const tag of tags) {
    add(tag.oid, { text: tag.name, kind: "tag" });
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
function shortDate(timestamp: number, offsetMinutes: number): string {
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
