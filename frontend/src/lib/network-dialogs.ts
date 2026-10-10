/** The Pull and Push dialogs' pure rules: what opens checked, what each button hands the
    engine, the command it will run, and what the repository remembers. */
import { splitUpstream } from "$lib/push-to";
import type { Branch } from "$lib/ipc";
import type { FetchOptions, NetworkDefaults, PullMethod, PullOptions, PushOptions, TagsMode } from "$lib/ipc/network-dialogs";

export const EMPTY_DEFAULTS: NetworkDefaults = {
  pullMethod: "merge",
  pullTags: false,
  pullNotes: false,
  pushTags: "none",
  pushNotes: false,
  pushSetUpstream: null,
  pushRemote: null,
};

export interface PullChoice {
  method: PullMethod;
  tags: boolean;
  notes: boolean;
}

export interface PushChoice {
  remote: string;
  local: string;
  branch: string;
  setUpstream: boolean;
  tags: TagsMode;
  notes: boolean;
  forceWithLease: boolean;
  /** Under the lease; on whenever the dialog opens. */
  forceIfIncludes: boolean;
  noVerify: boolean;
  /** The output window shows what would go; nothing is sent. Not an option of the engine's. */
  dryRun: boolean;
  atomic: boolean;
}

export function pullChoiceOf(defaults: NetworkDefaults): PullChoice {
  return { method: defaults.pullMethod, tags: defaults.pullTags, notes: defaults.pullNotes };
}

export function fetchOptionsOf(choice: PullChoice): FetchOptions {
  return { tags: choice.tags, notes: choice.notes };
}

/** `ffOnly` is Preferences ▸ Pull; it only shapes a merge. */
export function pullOptionsOf(choice: PullChoice, ffOnly: boolean): PullOptions {
  return {
    method: choice.method,
    ffOnly: choice.method === "merge" && ffOnly,
    fetch: fetchOptionsOf(choice),
    autostash: false,
  };
}

const notesFetch = (remote: string) => `git fetch +refs/notes/*:refs/notes-remote/${remote}/*`;

export function pullCommandLine(remote: string, choice: PullChoice, ffOnly: boolean): string {
  const options = pullOptionsOf(choice, ffOnly);
  const flags = ["pull", "--progress", "--prune"];
  if (choice.tags) flags.push("--tags", "--force");
  flags.push(options.method === "rebase" ? "--rebase" : options.ffOnly ? "--ff-only" : "--no-rebase", remote);
  const pull = `git ${flags.join(" ")}`;
  return choice.notes ? `${notesFetch(remote)}; ${pull}` : pull;
}

export function pushChoiceOf(
  defaults: NetworkDefaults,
  target: { remote: string; local: string; branch: string; hasUpstream: boolean },
): PushChoice {
  return {
    remote: target.remote,
    local: target.local,
    branch: target.branch,
    setUpstream: defaults.pushSetUpstream ?? !target.hasUpstream,
    tags: defaults.pushTags,
    notes: defaults.pushNotes,
    forceWithLease: false,
    forceIfIncludes: true,
    noVerify: false,
    dryRun: false,
    atomic: false,
  };
}

/** `leaseExpect`: the remote-tracking tip the dialog showed, for a lease without the check. */
export function pushOptionsOf(choice: PushChoice, leaseExpect: string | null = null): PushOptions {
  const { remote, local, branch, setUpstream, tags, notes, forceWithLease, forceIfIncludes, noVerify, atomic } = choice;
  return { remote, local, branch, setUpstream, tags, notes, forceWithLease, forceIfIncludes, leaseExpect, noVerify, atomic };
}

const BAD_BRANCH = /[\s~^:?*[\\]|\.\.|^[-/]|[/.]$|\.lock$|@\{/;

/** What makes Push unavailable, in words; `null` when it can run. */
export function pushProblem(choice: PushChoice): string | null {
  if (choice.remote === "") return "Choose a remote to push to.";
  if (choice.branch === "" || BAD_BRANCH.test(choice.branch)) {
    return "The branch name on the remote is empty or not a valid branch name.";
  }
  return null;
}

/** Branches `remote` has, by their short names, and the local name: a new branch. */
export function remoteBranchChoices(remote: string, remoteBranches: readonly string[], local: string): string[] {
  const names = new Set<string>([local]);
  for (const full of remoteBranches) {
    if (full.startsWith(`${remote}/`)) names.add(full.slice(remote.length + 1));
  }
  return [...names].sort();
}

export function tagsHint(mode: TagsMode): string {
  switch (mode) {
    case "none":
      return "Tags stay where they are";
    case "follow":
      return "Annotated tags that point at pushed commits (--follow-tags)";
    case "all":
      return "Push all local tags (--tags)";
  }
}

/** The half of the defaults that was asked to be remembered replaces its stored values;
    force is not a field of the defaults at all. */
export function mergeDefaults(
  stored: NetworkDefaults,
  remember: { pull?: PullChoice; push?: PushChoice },
): NetworkDefaults {
  const next = { ...stored };
  if (remember.pull) {
    next.pullMethod = remember.pull.method;
    next.pullTags = remember.pull.tags;
    next.pullNotes = remember.pull.notes;
  }
  if (remember.push) {
    next.pushTags = remember.push.tags;
    next.pushNotes = remember.push.notes;
    next.pushSetUpstream = remember.push.setUpstream;
    next.pushRemote = remember.push.remote;
  }
  return next;
}

/** Where a push of `branch` goes unless told otherwise: the remote the repository remembers
    while it exists, its triangular push target, else the push remote with the upstream's
    branch name when that is on the same remote. */
export function pushTargetOf(
  branch: { name: string; upstream: string | null; pushRemote: string | null; pushTarget: string | null },
  remotes: readonly string[],
  primary: string | null,
  remembered: string | null = null,
): { remote: string | null; branch: string } {
  const chosen = remembered !== null && remotes.includes(remembered) ? remembered : null;
  const target = branch.pushTarget ? splitUpstream(branch.pushTarget, remotes) : null;
  if (target && (chosen === null || target.remote === chosen)) return { remote: target.remote, branch: target.branch };
  const remote = chosen ?? (branch.pushRemote && remotes.includes(branch.pushRemote) ? branch.pushRemote : primary);
  const upstream = branch.upstream ? splitUpstream(branch.upstream, remotes) : null;
  return { remote, branch: upstream && upstream.remote === remote ? upstream.branch : branch.name };
}

/** A local branch as the Push dialog needs it: where it goes unless told otherwise. */
export type PushBranch = Pick<Branch, "name" | "upstream" | "pushRemote" | "pushTarget">;

export function pushBranchesOf(branches: readonly Branch[]): { locals: PushBranch[]; remoteBranches: string[] } {
  return {
    locals: branches
      .filter((b) => b.kind === "local")
      .map(({ name, upstream, pushRemote, pushTarget }) => ({ name, upstream, pushRemote, pushTarget })),
    remoteBranches: branches.filter((b) => b.kind === "remote").map((b) => b.name),
  };
}

/** The dialog after Local branch changed: where that branch goes, and Set as upstream as it
    would open for it. With nothing else to go by it stays on the remote chosen. */
export function chooseLocal(
  choice: PushChoice,
  branch: PushBranch,
  remotes: readonly string[],
  defaults: NetworkDefaults,
): PushChoice {
  const target = pushTargetOf(branch, remotes, choice.remote, defaults.pushRemote);
  return {
    ...choice,
    local: branch.name,
    remote: target.remote ?? choice.remote,
    branch: target.branch,
    setUpstream: defaults.pushSetUpstream ?? branch.upstream === null,
  };
}
