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
