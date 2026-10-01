/** The Pull and Push dialogs' pure rules: what opens checked, what each button hands the
    engine, the command it will run, and what the repository remembers. */
import { splitUpstream } from "$lib/push-to";
import type { FetchOptions, NetworkDefaults, PullMethod, PullOptions, PushOptions, TagsMode } from "$lib/ipc/network-dialogs";

export const EMPTY_DEFAULTS: NetworkDefaults = {
  pullMethod: "merge",
  pullTags: false,
  pullNotes: false,
  pushTags: "none",
  pushNotes: false,
  pushSetUpstream: null,
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
}

export function pullChoiceOf(defaults: NetworkDefaults): PullChoice {
  return { method: defaults.pullMethod, tags: defaults.pullTags, notes: defaults.pullNotes };
}

export function fetchOptionsOf(choice: PullChoice): FetchOptions {
  return { tags: choice.tags, notes: choice.notes };
}

/** `ffOnly` is Preferences ▸ Pull; it only shapes a merge. */
export function pullOptionsOf(choice: PullChoice, ffOnly: boolean): PullOptions {
  return { method: choice.method, ffOnly: choice.method === "merge" && ffOnly, fetch: fetchOptionsOf(choice) };
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
  };
}

export function pushOptionsOf(choice: PushChoice): PushOptions {
  return { ...choice };
}

export function pushCommandLine(choice: PushChoice): string {
  const flags = ["push", "--progress"];
  if (choice.setUpstream) flags.push("--set-upstream");
  if (choice.tags === "follow") flags.push("--follow-tags");
  if (choice.tags === "all") flags.push("--tags");
  if (choice.forceWithLease) flags.push("--force-with-lease", "--force-if-includes");
  flags.push(choice.remote, `refs/heads/${choice.local}:refs/heads/${choice.branch}`);
  const branch = `git ${flags.join(" ")}`;
  return choice.notes ? `${branch}; git push --progress ${choice.remote} refs/notes/*:refs/notes/*` : branch;
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
      return "Every tag, including ones nobody asked for (--tags)";
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
  }
  return next;
}

/** Where a push of `branch` goes unless told otherwise: its triangular push target, else
    the push remote with the upstream's branch name when that is on the same remote. */
export function pushTargetOf(
  branch: { name: string; upstream: string | null; pushRemote: string | null; pushTarget: string | null },
  remotes: readonly string[],
  primary: string | null,
): { remote: string | null; branch: string } {
  const target = branch.pushTarget ? splitUpstream(branch.pushTarget, remotes) : null;
  if (target) return { remote: target.remote, branch: target.branch };
  const remote = branch.pushRemote && remotes.includes(branch.pushRemote) ? branch.pushRemote : primary;
  const upstream = branch.upstream ? splitUpstream(branch.upstream, remotes) : null;
  return { remote, branch: upstream && upstream.remote === remote ? upstream.branch : branch.name };
}
