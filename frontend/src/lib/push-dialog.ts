/** The Push dialog's words and what its state means (F-610): the tracking hint, the status
    line, when nothing would go, the command it shows. */
import { relativeDate, shortOid } from "$lib/format";
import type { PushPreview } from "$lib/ipc/network-dialogs";
import type { PushChoice } from "$lib/network-dialogs";
import type { ConfirmRequest } from "$stores/confirm.svelte";

export interface UpstreamState {
  /** `already`: the branch tracks the target, and `--set-upstream` would change nothing. */
  kind: "already" | "other" | "none";
  hint: string;
}

export function upstreamState(upstream: string | null, remote: string, branch: string): UpstreamState {
  const target = `${remote}/${branch}`;
  if (upstream === target) return { kind: "already", hint: `Upstream: ${target}` };
  if (upstream === null) return { kind: "none", hint: `Track ${target} (no upstream yet)` };
  return { kind: "other", hint: `Track ${target} instead of ${upstream}` };
}

/** What the dialog sends: Set as upstream only where it changes something. */
export function effectiveChoice(choice: PushChoice, upstream: string | null): PushChoice {
  if (!choice.setUpstream || upstreamState(upstream, choice.remote, choice.branch).kind !== "already") return choice;
  return { ...choice, setUpstream: false };
}

export interface PushCommand {
  /** As a person would type it: `master` or `local:remote`, no `--progress`. */
  short: string;
  /** What the engine runs (`push_args`). */
  full: string;
}

/** Quoted, or a shell expands the `*` against files; double quotes, since cmd.exe hands single
    ones to git, which then matches no notes and pushes none. */
const NOTES = `"refs/notes/*:refs/notes/*"`;

function flagsOf(choice: PushChoice, remoteTip: string | null, ref: (branch: string) => string): string[] {
  const flags: string[] = [];
  if (choice.setUpstream) flags.push("--set-upstream");
  if (choice.tags === "follow") flags.push("--follow-tags");
  if (choice.tags === "all") flags.push("--tags");
  if (choice.noVerify) flags.push("--no-verify");
  if (choice.atomic) flags.push("--atomic");
  if (choice.forceWithLease) {
    const name = ref(choice.branch);
    if (choice.forceIfIncludes) flags.push(`--force-with-lease=${name}`, "--force-if-includes");
    else flags.push(`--force-with-lease=${name}:${remoteTip ?? ""}`);
  }
  return flags;
}

export function pushCommand(choice: PushChoice, remoteTip: string | null): PushCommand {
  const dry = choice.dryRun ? ["--dry-run"] : [];
  const notes = choice.notes ? [NOTES] : [];
  const refspec = choice.local === choice.branch ? choice.local : `${choice.local}:${choice.branch}`;
  const short = ["git push", ...dry, ...flagsOf(choice, remoteTip, (b) => b), choice.remote, refspec, ...notes].join(" ");
  const full = [
    "git push",
    ...dry,
    "--progress",
    ...flagsOf(choice, remoteTip, (b) => `refs/heads/${b}`),
    choice.remote,
    `refs/heads/${choice.local}:refs/heads/${choice.branch}`,
    ...notes,
  ].join(" ");
  return { short, full };
}

/** What a lease is checked against, the tip written out; `note` says what that means. */
export function leaseLine(remote: string, branch: string, remoteTip: string | null): { flag: string; note: string } {
  if (remoteTip === null) return { flag: `--force-with-lease=${branch}:`, note: `${branch} must not exist on ${remote}` };
  return { flag: `--force-with-lease=${branch}:${shortOid(remoteTip)}`, note: `${remote}/${branch} as last fetched` };
}

/** Preferences ▸ Push: names separated by commas or spaces, each once. */
export function protectedNames(text: string): string[] {
  return [...new Set(text.split(/[\s,]+/).filter((name) => name !== ""))];
}

export function protectedQuestion(remote: string, branch: string): ConfirmRequest {
  return {
    title: "Force push to a protected branch?",
    message: `${branch} on ${remote} is a protected branch (Preferences ▸ Push). A force push rewrites its history for everyone who fetched it.`,
    confirm: "Force push",
    warning: true,
  };
}

/** The branch against its remote-tracking one, as of the last fetch. */
export function pushStatus(preview: PushPreview, remote: string): string {
  if (preview.remoteTip === null) return `New branch on ${remote}`;
  const { total: ahead, behind } = preview;
  if (ahead > 0 && behind > 0) return `${ahead} ahead, ${behind} behind — diverged`;
  if (behind > 0) return `${behind} behind`;
  if (ahead > 0) return `${ahead} ahead`;
  return "Up to date";
}

/** Both in Unix ms. */
export function fetchedText(fetchedAt: number | null, now: number): string {
  if (fetchedAt === null) return "never fetched";
  return `fetched ${relativeDate(Math.floor(fetchedAt / 1000), 0, Math.floor(now / 1000)).toLowerCase()}`;
}

const HOUR_MS = 3_600_000;

/** The status as of the last fetch and how long ago that was; `stale` past `staleHours`
    (Preferences ▸ Push) or with nothing ever fetched, when the status may no longer hold.
    `fetchedHere`: when the dialog's own Fetch of this remote ended, which git may not record. */
export function statusLine(
  preview: PushPreview,
  remote: string,
  now: number,
  staleHours: number,
  fetchedHere: number | null = null,
): { text: string; stale: boolean } {
  const status = pushStatus(preview, remote);
  const fetchedAt = fetchedHere === null ? preview.fetchedAt : Math.max(preview.fetchedAt ?? 0, fetchedHere);
  if (fetchedAt === null) return { text: `${status} · never fetched`, stale: true };
  if (now - fetchedAt <= staleHours * HOUR_MS) return { text: `${status} · ${fetchedText(fetchedAt, now)}`, stale: false };
  const ago = relativeDate(Math.floor(fetchedAt / 1000), 0, Math.floor(now / 1000)).toLowerCase();
  return { text: `${status} as of last fetch (${ago})`, stale: true };
}

const STALE_PRESETS: readonly (readonly [number, string])[] = [
  [1, "1 hour"],
  [6, "6 hours"],
  [12, "12 hours"],
  [24, "1 day"],
  [72, "3 days"],
  [168, "1 week"],
  [720, "30 days"],
];

/** Preferences ▸ Push: the thresholds offered, in hours, with `current` among them. */
export function staleChoices(current: number): [string, string][] {
  const choices = STALE_PRESETS.some(([hours]) => hours === current)
    ? [...STALE_PRESETS]
    : [...STALE_PRESETS, [current, `${current} hours`] as const].sort((a, b) => a[0] - b[0]);
  return choices.map(([hours, label]) => [String(hours), label]);
}

/** The tags a push would send, asked of the remote in the background. */
export type TagCount = { state: "counting" } | { state: "done"; tags: string[] } | { state: "failed"; reason: string };

/** The gray count beside the tags choice; `null` with no tags chosen. */
export function tagCounter(count: TagCount | null): { text: string; tip?: string } | null {
  switch (count?.state) {
    case undefined:
      return null;
    case "counting":
      return { text: "· counting…" };
    case "failed":
      return { text: "· could not count", tip: `Could not count tags: ${count.reason}` };
    case "done":
      return count.tags.length === 0
        ? { text: "· 0 to push" }
        : { text: `· ${count.tags.length} to push`, tip: count.tags.join("\n") };
  }
}

/** Beside Push notes: local notes the remote lacks, as of its notes last fetched. */
export function notesNote(preview: PushPreview | null): string | undefined {
  if (!preview?.hasLocalNotes || preview.notesUnpushed === 0) return undefined;
  return preview.notesUnpushed === null ? "· not compared with the remote" : `· ${preview.notesUnpushed} not pushed`;
}

export function commitsText(preview: PushPreview | null): string {
  if (preview === null) return "Counting commits…";
  if (preview.total === 0) return "No commits to push";
  return `${preview.total} ${preview.total === 1 ? "commit" : "commits"} to push`;
}

type Goes = "yes" | "maybe" | "no";

/** The branch moves: commits to add, a branch to create, or a forced move (even back). */
function commitsGo(preview: PushPreview, choice: PushChoice): Goes {
  const moves =
    preview.total > 0 || preview.remoteTip === null || (choice.forceWithLease && preview.remoteTip !== preview.localTip);
  return moves ? "yes" : "no";
}

function tagsGo(choice: PushChoice, tags: TagCount | null): Goes {
  if (choice.tags === "none") return "no";
  if (tags?.state !== "done") return "maybe";
  return tags.tags.length > 0 ? "yes" : "no";
}

function notesGo(preview: PushPreview, choice: PushChoice): Goes {
  if (!choice.notes || !preview.hasLocalNotes || preview.notesUnpushed === 0) return "no";
  return preview.notesUnpushed === null ? "maybe" : "yes";
}

/** Refs the push sends: the branch when it moves (not known yet: it does), each counted tag
    (not counted yet: one), the notes. */
function refsGoing(preview: PushPreview | null, choice: PushChoice, tags: TagCount | null): number {
  const branch = preview === null || commitsGo(preview, choice) === "yes" ? 1 : 0;
  const tagRefs = choice.tags === "none" ? 0 : tags?.state === "done" ? tags.tags.length : 1;
  const notes = preview !== null && notesGo(preview, choice) !== "no" ? 1 : 0;
  return branch + tagRefs + notes;
}

/** Atomic holds two refs or more together, where the remote takes it: `support` is what it
    said (`null` or not asked yet: unknown). */
export function atomicGate(
  preview: PushPreview | null,
  choice: PushChoice,
  tags: TagCount | null,
  support: boolean | null | undefined,
): { enabled: boolean; tip: string } {
  if (support === false) return { enabled: false, tip: "Remote does not support atomic push" };
  if (refsGoing(preview, choice, tags) < 2) {
    return { enabled: false, tip: "Needs two refs or more: the branch with its tags or notes" };
  }
  return { enabled: true, tip: "Every ref is updated, or none is" };
}

/** The Push button: what it says it sends, and whether anything would go at all. Set upstream
    rides along unnamed unless it is all there is; what is not counted yet counts as going. */
export function pushButton(
  preview: PushPreview | null,
  choice: PushChoice,
  upstream: string | null,
  tags: TagCount | null,
): { label: string; nothing: boolean } {
  const verb = choice.forceWithLease ? "Force push" : "Push";
  if (preview === null) return { label: choice.dryRun ? "Dry run" : verb, nothing: false };
  const kinds = { commits: commitsGo(preview, choice), tags: tagsGo(choice, tags), notes: notesGo(preview, choice) };
  const sure = (Object.keys(kinds) as (keyof typeof kinds)[]).filter((kind) => kinds[kind] === "yes");
  const maybe = Object.values(kinds).includes("maybe");
  const upstreamOnly = sure.length === 0 && !maybe && effectiveChoice(choice, upstream).setUpstream;
  const nothing = sure.length === 0 && !maybe && !upstreamOnly;
  if (choice.dryRun) return { label: "Dry run", nothing };
  if (upstreamOnly) return { label: "Set upstream", nothing };
  if (sure.length !== 1) return { label: verb, nothing };
  const { total } = preview;
  const named = {
    commits: total > 0 ? `${verb} ${total} ${total === 1 ? "commit" : "commits"}` : verb,
    tags: "Push tags",
    notes: "Push notes",
  };
  return { label: named[sure[0]!], nothing };
}
