/** The Push dialog's words and what its state means (F-610): the tracking hint, the status
    line, when nothing would go, the command it shows. */
import type { PushChoice } from "$lib/network-dialogs";

export interface UpstreamState {
  /** `already`: the branch tracks the target, and `--set-upstream` would change nothing. */
  kind: "already" | "other" | "none";
  hint: string;
}

export function upstreamState(upstream: string | null, remote: string, branch: string): UpstreamState {
  const target = `${remote}/${branch}`;
  if (upstream === target) return { kind: "already", hint: `Already tracking ${target}` };
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
  /** What the engine runs (`push_args`), then the notes. */
  full: string;
}

const NOTES = "refs/notes/*:refs/notes/*";

function flagsOf(choice: PushChoice): string[] {
  const flags: string[] = [];
  if (choice.setUpstream) flags.push("--set-upstream");
  if (choice.tags === "follow") flags.push("--follow-tags");
  if (choice.tags === "all") flags.push("--tags");
  if (choice.forceWithLease) flags.push("--force-with-lease", "--force-if-includes");
  return flags;
}

export function pushCommand(choice: PushChoice): PushCommand {
  const flags = flagsOf(choice);
  const refspec = choice.local === choice.branch ? choice.local : `${choice.local}:${choice.branch}`;
  const short = ["git push", ...flags, choice.remote, refspec].join(" ");
  const full = ["git push --progress", ...flags, choice.remote, `refs/heads/${choice.local}:refs/heads/${choice.branch}`].join(" ");
  if (!choice.notes) return { short, full };
  return {
    short: `${short}; git push ${choice.remote} ${NOTES}`,
    full: `${full}; git push --progress ${choice.remote} ${NOTES}`,
  };
}
