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
  /** A button that does the fix, when Cogit can. */
  action?: HealthAction;
}

export interface HealthAction {
  id: string;
  label: string;
  /** The submodules it acts in, as paths from the repository that was checked. */
  targets: string[];
}

export const FETCH_MODULES = "fetch-modules";
export const TRUST_DIRECTORY = "trust-directory";
export const RUN_GC = "run-gc";
export const WRITE_COMMIT_GRAPH = "write-commit-graph";
export const CLEAR_GC_LOCK = "clear-gc-lock";

/** The confirmation a maintenance button asks before it runs (E2). */
export const MAINTENANCE: Record<string, { task: "gc" | "commitGraph" | "clearGcLock"; question: string }> = {
  [RUN_GC]: {
    task: "gc",
    question: "Run git gc? It packs loose objects and prunes unreachable ones older than two weeks; it can take a while on a large repository.",
  },
  [WRITE_COMMIT_GRAPH]: {
    task: "commitGraph",
    question: "Write the commit-graph file? It only adds an index that speeds up history walks.",
  },
  [CLEAR_GC_LOCK]: {
    task: "clearGcLock",
    question: "Remove gc.pid? Do this only if no git gc is running for this repository.",
  },
};

function bytes(count: number): string {
  if (count >= 1 << 30) return `${(count / (1 << 30)).toFixed(1)} GB`;
  if (count >= 1 << 20) return `${(count / (1 << 20)).toFixed(1)} MB`;
  return `${Math.round(count / 1024)} KB`;
}

export function placeOf(repoName: string, module: string): string {
  return module === "" ? repoName : `${repoName} [${module}]`;
}

/** Broken paths first: a wrong setting only bites on a rename, a missing folder always. */
const SEVERITY: Record<HealthIssue["kind"], number> = {
  danglingModule: 0,
  danglingWorktree: 1,
  missingModuleCommit: 2,
  ignoreCaseMismatch: 3,
  unsafeDirectory: -1,
  replacedHistory: 4,
  lfsPointers: 5,
  normalizationTwins: 6,
  precomposeUnicodeOff: 7,
  staleGcLock: 8,
  housekeepingDue: 9,
  noCommitGraph: 10,
  sparseCheckout: 11,
};

function idOf(issue: HealthIssue): string {
  switch (issue.kind) {
    case "ignoreCaseMismatch":
      return `ignoreCaseMismatch:${issue.configured}`;
    case "danglingModule":
    case "danglingWorktree":
      return `${issue.kind}:${issue.foreign ? "foreign" : "gone"}`;
    case "lfsPointers":
      return `lfsPointers:${issue.installed}`;
    case "missingModuleCommit":
    case "unsafeDirectory":
    case "replacedHistory":
    case "housekeepingDue":
    case "noCommitGraph":
    case "staleGcLock":
    case "sparseCheckout":
    case "normalizationTwins":
    case "precomposeUnicodeOff":
      return issue.kind;
  }
}

function detailOf(issue: HealthIssue): string | undefined {
  switch (issue.kind) {
    case "ignoreCaseMismatch":
      return undefined;
    case "missingModuleCommit":
      return issue.commit.slice(0, 10);
    case "danglingModule":
    case "danglingWorktree":
      return issue.target;
    case "unsafeDirectory":
      return issue.path;
    case "replacedHistory":
      return `${issue.count} replacement(s)`;
    case "housekeepingDue":
      return `${issue.loose} loose object(s), ${issue.packs} pack(s), ${bytes(issue.packBytes)}`;
    case "staleGcLock":
      return `gc.pid is ${issue.hours} h old`;
    case "sparseCheckout":
      return `${issue.patterns} pattern(s)${issue.cone ? ", cone mode" : ""}`;
    case "lfsPointers":
      return `${issue.count} file(s): ${issue.sample.join(", ")}`;
    case "normalizationTwins":
      return issue.paths.join(", ");
    case "noCommitGraph":
    case "precomposeUnicodeOff":
      return undefined;
  }
}

const DOCS = {
  ignoreCase: "https://git-scm.com/docs/git-config#Documentation/git-config.txt-coreignoreCase",
  submodule: "https://git-scm.com/docs/git-submodule#Documentation/git-submodule.txt-absorbgitdirs",
  worktree: "https://git-scm.com/docs/git-worktree#Documentation/git-worktree.txt-repair",
  fetch: "https://git-scm.com/docs/git-fetch",
  replace: "https://git-scm.com/docs/git-replace",
  safeDirectory: "https://git-scm.com/docs/git-config#Documentation/git-config.txt-safedirectory",
  gc: "https://git-scm.com/docs/git-gc",
  commitGraph: "https://git-scm.com/docs/git-commit-graph",
  sparse: "https://git-scm.com/docs/git-sparse-checkout",
  lfs: "https://git-lfs.com",
  precompose: "https://git-scm.com/docs/git-config#Documentation/git-config.txt-coreprecomposeUnicode",
};

function describe(issue: HealthIssue): Omit<HealthWarning, "id" | "places"> {
  switch (issue.kind) {
    case "housekeepingDue":
      return {
        title: "The repository needs housekeeping",
        body:
          "There are more loose objects or packs than gc.auto and gc.autoPackLimit allow, so " +
          "every command reads more files than it needs to. git gc packs them together.",
        docs: DOCS.gc,
        fixes: ["git count-objects -v", "git gc"],
        action: { id: RUN_GC, label: "Run gc…", targets: [] },
      };
    case "noCommitGraph":
      return {
        title: "The repository has no commit-graph",
        body:
          "Without a commit-graph, history walks (the log, ahead/behind counts, merge bases) " +
          "read every commit object. Writing one changes no commit.",
        docs: DOCS.commitGraph,
        fixes: ["git commit-graph write --reachable --changed-paths"],
        action: { id: WRITE_COMMIT_GRAPH, label: "Write commit-graph…", targets: [] },
      };
    case "staleGcLock":
      return {
        title: "A stale gc lock blocks automatic maintenance",
        body:
          `gc.pid was written ${issue.hours} hours ago by a git gc that did not finish. While ` +
          "it is there, every automatic gc is skipped. Remove it only if no gc is running.",
        docs: DOCS.gc,
        fixes: ["rm .git/gc.pid"],
        action: { id: CLEAR_GC_LOCK, label: "Remove gc.pid…", targets: [] },
      };
    case "sparseCheckout":
      return {
        title: "Sparse checkout is on",
        body:
          "Only the files the sparse patterns select are in the working tree. The others are " +
          "hidden, not deleted: with the skip-worktree switch on, Files lists them as " +
          "'Outside sparse checkout' (◌), and a commit leaves them as they are.",
        docs: DOCS.sparse,
        fixes: ["git sparse-checkout list", "git sparse-checkout disable"],
      };
    case "lfsPointers":
      return issue.installed
        ? {
            title: "Git LFS files are not downloaded",
            body:
              "These files are tracked through Git LFS, but the working tree holds their " +
              "pointers, not their content. Pulling the LFS objects replaces the pointers.",
            docs: DOCS.lfs,
            fixes: ["git lfs pull"],
          }
        : {
            title: "Git LFS is not installed",
            body:
              "This repository tracks files through Git LFS, and without it the working tree " +
              "holds small pointer files instead of their content. A commit would record the " +
              "pointers as they are, and nothing is lost, but the files cannot be used.",
            docs: DOCS.lfs,
            fixes: ["git lfs install", "git lfs pull"],
          };
    case "normalizationTwins":
      return {
        title: "File names differ only in Unicode normalization",
        body:
          "The index holds the same name twice, once composed (NFC) and once decomposed " +
          "(NFD). macOS and Windows treat them as one file, so a checkout there keeps only one " +
          "and shows the other as changed. Rename one of them.",
        docs: DOCS.precompose,
        fixes: ["git mv <one-form> <another-name>"],
      };
    case "precomposeUnicodeOff":
      return {
        title: "Git config option 'core.precomposeUnicode' is off",
        body:
          "macOS stores file names decomposed (NFD). With core.precomposeUnicode false, Git " +
          "records such names as they come from the disk, and on Linux and Windows they are " +
          "different names from what was typed.",
        docs: DOCS.precompose,
        fixes: ["git config core.precomposeUnicode true"],
      };
    case "replacedHistory":
      return {
        title: "Part of this repository's history is replaced",
        body:
          "refs/replace/ swaps some commits or objects for others, so the graph, diffs and " +
          "blame show the replacement rather than what is stored. Pushes and clones do not " +
          "carry replacements unless they are pushed explicitly.",
        docs: DOCS.replace,
        fixes: ["git replace --list", "git --no-replace-objects log"],
      };
    case "unsafeDirectory":
      return {
        title: "Git does not trust this repository's folder",
        body:
          "The folder is owned by another user and is not listed in safe.directory, so Git " +
          "refuses to run in it (\"detected dubious ownership\"). Commits, fetches and every " +
          "other command fail until the folder is trusted.",
        docs: DOCS.safeDirectory,
        fixes: [`git config --global --add safe.directory ${issue.path}`],
        action: { id: TRUST_DIRECTORY, label: "Trust this folder", targets: [] },
      };
    case "missingModuleCommit":
      return {
        title: "Submodule commit is not available locally",
        body:
          "The parent records a commit that the submodule's repository does not have: it has " +
          "not been fetched yet. Until it is, the submodule cannot be compared with it and a " +
          "checkout of the recorded commit fails. Fetching in the submodule brings it.",
        docs: DOCS.fetch,
        fixes: ["git fetch"],
        action: { id: FETCH_MODULES, label: "Fetch in submodule", targets: [] },
      };
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
          : "The folder the submodule's .git file names is gone. Initializing the submodule " +
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
      held.action?.targets.push(finding.module);
      continue;
    }
    const described = describe(finding.issue);
    if (described.action) described.action = { ...described.action, targets: [finding.module] };
    byId.set(id, {
      id,
      ...described,
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
