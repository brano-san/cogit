import { shortOid } from "$lib/format";
import type { RepoState, RepoStatus } from "$lib/ipc";

export type BannerAction = "continue" | "skip" | "abort" | "createBranch";

export interface Banner {
  title: string;
  detail: string;
  severity: "info" | "warning" | "error";
  actions: BannerAction[];
}

const INTERRUPTED: Partial<Record<RepoState["kind"], string>> = {
  merging: "Merge",
  rebasing: "Rebase",
  cherryPicking: "Cherry-pick",
  reverting: "Revert",
  bisecting: "Bisect",
  applyingPatches: "Applying patches",
};

/** What the Working Tree row and the Repositories tree call a state that outlasts a command. */
const ONGOING: Partial<Record<RepoState["kind"], string>> = {
  merging: "merging",
  rebasing: "rebasing",
  cherryPicking: "cherry-picking",
  reverting: "reverting",
  bisecting: "bisecting",
  applyingPatches: "applying patches",
};

/** Merge and bisect have nothing to skip; bisect ends with `reset`, never `--continue`. */
function interruptedActions(kind: RepoState["kind"]): BannerAction[] {
  if (kind === "merging") return ["continue", "abort"];
  if (kind === "bisecting") return ["abort"];
  return ["continue", "skip", "abort"];
}

export function workingTreeLabel(status: RepoStatus | undefined, state: RepoState | undefined): string {
  const parts: string[] = [];
  if (status && status.staged > 0) parts.push(`${status.staged} staged`);
  if (status && status.unstaged > 0) parts.push(`${status.unstaged} modified`);
  if (status && status.untracked > 0) parts.push(`${status.untracked} untracked`);
  if (status && status.conflicted > 0) parts.push(`${status.conflicted} conflicted`);
  const counts = !status ? "Working Tree" : parts.length > 0 ? `Working Tree (${parts.join(", ")})` : "Working Tree — clean";
  const ongoing = state ? ONGOING[state.kind] : undefined;
  return ongoing ? `${counts}, ${ongoing}` : counts;
}

export const STATE_TAG_HINT =
  "Stopped half way or on a detached HEAD: the banner in the Graph panel says what to do.";

/** `<merging>` beside a name in Repositories. A submodule is detached by design (R-130). */
export function repoStateTag(state: RepoState | null | undefined, submodule = false): string | null {
  if (!state) return null;
  if (state.kind === "detachedHead") return submodule ? null : "<detached>";
  const ongoing = ONGOING[state.kind];
  return ongoing ? `<${ongoing}>` : null;
}

/** `submodule` changes one thing: a detached HEAD there is how submodules work, not a
    situation to be rescued from (doc/12-risks.md, R-130). */
export function stateBanner(
  state: RepoState,
  indexLock: string | null,
  submodule = false,
): Banner | null {
  // A stale lock blocks every write, so it outranks whatever else is going on.
  if (indexLock) {
    return {
      title: "The index is locked",
      detail: `${indexLock} — another Git process may still be running. Delete it only once you are sure none is.`,
      severity: "error",
      actions: [],
    };
  }

  const interrupted = INTERRUPTED[state.kind];
  if (interrupted) {
    return {
      title: `${interrupted} in progress`,
      detail: "Resolve the conflicts and continue, or abort to go back.",
      severity: "warning",
      actions: interruptedActions(state.kind),
    };
  }

  if (state.kind === "detachedHead") {
    if (submodule) {
      return {
        title: "Detached HEAD",
        detail: `On commit ${shortOid(state.oid)}, the one the parent repository records.`,
        severity: "info",
        actions: [],
      };
    }
    return {
      title: "Detached HEAD",
      detail: `On commit ${shortOid(state.oid)}. New commits belong to no branch and are easy to lose.`,
      severity: "warning",
      actions: ["createBranch"],
    };
  }

  if (state.kind === "empty") {
    return {
      title: "No commits yet",
      detail: "Stage a file and make the first commit.",
      severity: "info",
      actions: [],
    };
  }

  if (state.kind === "bare") {
    return {
      title: "Bare repository",
      detail: "There is no working tree here, so nothing can be staged or committed.",
      severity: "info",
      actions: [],
    };
  }

  return null;
}
