import type { RepoState } from "$lib/ipc";

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
};

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
      actions: state.kind === "merging" ? ["continue", "abort"] : ["continue", "skip", "abort"],
    };
  }

  if (state.kind === "detachedHead") {
    if (submodule) {
      return {
        title: "Detached HEAD",
        detail: `On commit ${state.oid.slice(0, 7)}, the one the parent repository records.`,
        severity: "info",
        actions: [],
      };
    }
    return {
      title: "Detached HEAD",
      detail: `On commit ${state.oid.slice(0, 7)}. New commits belong to no branch and are easy to lose.`,
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
