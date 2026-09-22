import type { HealthFinding, HealthIssue } from "$lib/ipc";

export interface HealthPlace {
  label: string;
  /** Where a broken path points, for the row it belongs to. */
  detail?: string;
}

export interface HealthWarning {
  /** Stable across opens and independent of how many places it covers: Ignore keys on it. */
  id: string;
  title: string;
  body: string;
  docs: string;
  places: HealthPlace[];
  /** Commands, each run inside the repository or submodule it is listed next to. */
  fixes: string[];
}

export function placeOf(repoName: string, module: string): string {
  return module === "" ? repoName : `${repoName} [${module}]`;
}

/** Broken paths first: a wrong setting only bites on a rename, a missing folder always. */
const SEVERITY: Record<HealthIssue["kind"], number> = {
  danglingModule: 0,
  danglingWorktree: 1,
  ignoreCaseMismatch: 2,
};

function idOf(issue: HealthIssue): string {
  switch (issue.kind) {
    case "ignoreCaseMismatch":
      return `ignoreCaseMismatch:${issue.configured}`;
    case "danglingModule":
    case "danglingWorktree":
      return `${issue.kind}:${issue.foreign ? "foreign" : "gone"}`;
  }
}

function detailOf(issue: HealthIssue): string | undefined {
  return issue.kind === "ignoreCaseMismatch" ? undefined : issue.target;
}

const DOCS = {
  ignoreCase: "https://git-scm.com/docs/git-config#Documentation/git-config.txt-coreignoreCase",
  submodule: "https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-absorbgitdirs",
  worktree: "https://git-scm.com/docs/git-worktree#Documentation/git-worktree.txt-repair",
};

function describe(issue: HealthIssue): Omit<HealthWarning, "id" | "places"> {
  switch (issue.kind) {
    case "ignoreCaseMismatch":
      return issue.actual
        ? {
            title: "Git config option 'core.ignoreCase' does not match your file system",
            body:
              "core.ignoreCase is false, but this folder does not tell upper case from lower " +
              "case. A rename that changes only case (Readme.md → README.md) will then be seen " +
              "as two files, and a checkout can fail or keep both. This is what a repository " +
              "cloned on Linux looks like when it is used from Windows.",
            docs: DOCS.ignoreCase,
            fixes: ["git config core.ignoreCase true"],
          }
        : {
            title: "Git config option 'core.ignoreCase' does not match your file system",
            body:
              "core.ignoreCase is true, but this folder tells upper case from lower case. Git " +
              "will then miss files that differ only in case, and a rename that changes only " +
              "case will not be recorded.",
            docs: DOCS.ignoreCase,
            fixes: ["git config core.ignoreCase false"],
          };
    case "danglingModule":
      return {
        title: issue.foreign
          ? "A submodule's .git file holds a path from another operating system"
          : "A submodule's .git file points to a folder that does not exist",
        body: issue.foreign
          ? "The submodule's .git file names its repository by an absolute path written on " +
            "another system, so Git cannot find it here. Absorbing the git directory into the " +
            "parent rewrites the path as a relative one that works on both."
          : "The folder the submodule's .git file names is gone. Initialising the submodule " +
            "again recreates it.",
        docs: DOCS.submodule,
        fixes: issue.foreign
          ? ["git submodule absorbgitdirs"]
          : ["git submodule deinit -f -- <path>", "git submodule update --init -- <path>"],
      };
    case "danglingWorktree":
      return issue.foreign
        ? {
            title: "A worktree is registered at a path from another operating system",
            body:
              `Worktree '${issue.name}' was registered on another system, so its path does ` +
              "not exist here. Repair tells Git where the worktree folder is on this machine.",
            docs: DOCS.worktree,
            fixes: ["git worktree repair <path-to-the-worktree-here>"],
          }
        : {
            title: "A worktree folder no longer exists",
            body:
              `Worktree '${issue.name}' is still registered, but its folder is gone. Pruning ` +
              "forgets it; nothing on disk is touched.",
            docs: DOCS.worktree,
            fixes: ["git worktree prune"],
          };
  }
}

/** One warning per problem, however many repositories and submodules share it. */
export function groupFindings(findings: readonly HealthFinding[], repoName: string): HealthWarning[] {
  const byId = new Map<string, HealthWarning & { rank: number }>();
  for (const finding of findings) {
    const id = idOf(finding.issue);
    const place: HealthPlace = { label: placeOf(repoName, finding.module) };
    const detail = detailOf(finding.issue);
    if (detail !== undefined) place.detail = detail;

    const held = byId.get(id);
    if (held) {
      held.places.push(place);
      continue;
    }
    byId.set(id, {
      id,
      ...describe(finding.issue),
      places: [place],
      rank: SEVERITY[finding.issue.kind],
    });
  }
  return [...byId.values()]
    .sort((a, b) => a.rank - b.rank)
    .map(({ rank: _rank, ...warning }) => warning);
}

export function visibleWarnings(
  warnings: readonly HealthWarning[],
  ignored: ReadonlySet<string>,
  later: ReadonlySet<string>,
): HealthWarning[] {
  return warnings.filter((warning) => !ignored.has(warning.id) && !later.has(warning.id));
}
