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

/** Quoted: a shell would expand the `*` against files. */
const NOTES = "'refs/notes/*:refs/notes/*'";

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
    (Preferences ▸ Push) or with nothing ever fetched, when the status may no longer hold. */
export function statusLine(
  preview: PushPreview,
  remote: string,
  now: number,
  staleHours: number,
): { text: string; stale: boolean } {
  const status = pushStatus(preview, remote);
  const { fetchedAt } = preview;
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

export function tagCountText(count: TagCount): string {
  switch (count.state) {
    case "counting":
      return "Counting tags…";
    case "failed":
      return `Could not count tags: ${count.reason}`;
    case "done": {
      const n = count.tags.length;
      return n === 0 ? "No tags will be pushed" : `${n} ${n === 1 ? "tag" : "tags"} will be pushed`;
    }
  }
}

export function commitsText(preview: PushPreview | null): string {
  if (preview === null) return "Counting commits…";
  if (preview.total === 0) return "No commits to push";
  return `${preview.total} ${preview.total === 1 ? "commit" : "commits"} to push`;
}

/** Nothing at all would go: the remote has the branch's commit, and no tag, note or upstream
    change rides along. Not known yet counts as something. */
export function isUpToDate(preview: PushPreview, choice: PushChoice, upstream: string | null, tags: TagCount | null): boolean {
  if (preview.remoteTip === null || preview.remoteTip !== preview.localTip) return false;
  if (effectiveChoice(choice, upstream).setUpstream) return false;
  if (choice.tags !== "none" && !(tags?.state === "done" && tags.tags.length === 0)) return false;
  return !(choice.notes && preview.hasLocalNotes && preview.notesUnpushed !== 0);
}

export function pushButtonLabel(choice: PushChoice, total: number): string {
  if (choice.dryRun) return "Dry run";
  const verb = choice.forceWithLease ? "Force push" : "Push";
  return total === 0 ? verb : `${verb} ${total} ${total === 1 ? "commit" : "commits"}`;
}
