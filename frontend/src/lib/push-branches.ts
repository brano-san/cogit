import { initialRemote, pushRefspec, tracksByDefault, type PushSource } from "./push-to";

export interface PushBranch {
  name: string;
  upstream: string | null;
}

const asSource = (branch: PushBranch): PushSource => ({ kind: "branch", name: branch.name, upstream: branch.upstream });

/** The remote the dialog opens on: the first branch's, as Push To of that branch would. */
export function startRemote(
  branches: readonly PushBranch[],
  remotes: readonly string[],
  primary: string | null,
): string | null {
  const first = branches[0];
  return first ? initialRemote(asSource(first), remotes, primary) : (primary ?? remotes[0] ?? null);
}

/** One push per branch, each to its tracked or same-named branch on `remote`. A branch that
    tracks nothing starts tracking when `track` is on; the others keep their upstream. */
export function branchPushes(
  branches: readonly PushBranch[],
  remote: string,
  remotes: readonly string[],
  track: boolean,
): { refspec: string; track: boolean }[] {
  return branches.map((branch) => ({
    refspec: pushRefspec(asSource(branch), { mode: "tracked" }, remote, remotes),
    track: track && tracksByDefault(asSource(branch)),
  }));
}

export function pushBranchesTitle(count: number, remote: string): string {
  return `Push ${count} ${count === 1 ? "branch" : "branches"} to remote '${remote || "—"}'`;
}
