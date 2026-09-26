import type { CloneDestination, CloneRequest, RemoteBranches } from "$lib/ipc/clone";

/** Repository ▸ Clone…, SmartGit's three pages (F-575). */
export const CLONE_PAGES = ["repository", "selection", "directory"] as const;
export type ClonePage = (typeof CLONE_PAGES)[number];

export const PAGE_TITLES: Record<ClonePage, string> = {
  repository: "Repository",
  selection: "Selection",
  directory: "Directory",
};

/** `D:\src`, `\\server\share`, `/home/me`: a folder named in full. */
export function isFullPath(path: string): boolean {
  return /^[a-zA-Z]:[\\/]/.test(path) || /^[\\/]{2}[^\\/]/.test(path) || path.startsWith("/");
}

/** `scheme://…` or the SSH form `[user@]host:path`. A drive letter is one character, so
    `C:\src` is a folder. */
export function isUrl(source: string): boolean {
  return /^[a-z][a-z0-9+.-]*:\/\//i.test(source) || /^([^@\s/\\:]+@)?[^@\s/\\:]{2,}:(?![\\/]{2})/.test(source);
}

export function sourceProblem(source: string): string | null {
  const text = source.trim();
  if (text === "") return "Enter the URL or the folder of a repository";
  if (text.startsWith("-")) return "A repository cannot start with “-”";
  if (!isUrl(text) && !isFullPath(text)) return "Enter a URL, or the full path of a folder";
  return null;
}

export const LIMIT_MAX_MB = 1_000_000;

export function limitProblem(skip: boolean, megabytes: string): string | null {
  if (!skip) return null;
  const text = megabytes.trim();
  const value = Number(text);
  if (/^\d+$/.test(text) && value >= 1 && value <= LIMIT_MAX_MB) return null;
  return "Enter the size limit in whole megabytes, 1 or more";
}

/** In the parent's own separator, so a Windows path stays one. */
export function joinPath(parent: string, name: string): string {
  const base = parent.trim();
  if (base === "") return name;
  if (/[\\/]$/.test(base)) return base + name;
  return base + (base.includes("\\") ? "\\" : "/") + name;
}

/** The folder `root` sits in: a clone goes beside the repository in front by default. */
export function parentFolder(root: string): string {
  const trimmed = root.replace(/[\\/]+$/, "");
  const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (cut <= 0) return cut === 0 ? trimmed.slice(0, 1) : "";
  const parent = trimmed.slice(0, cut);
  return /^[a-zA-Z]:$/.test(parent) ? parent + trimmed[cut] : parent;
}

const FORBIDDEN_IN_NAME = /[\\/:*?"<>|]/;

export interface Destination {
  path: string;
  kind: CloneDestination;
}

export interface DirectoryChoice {
  parent: string;
  name: string;
  /** The last answer about a path; one for another path is still being replaced. */
  seen: Destination | null;
}

/** What keeps Finish inactive, or `null`. The folder may be missing or empty, as for git. */
export function directoryProblem(choice: DirectoryChoice): string | null {
  const parent = choice.parent.trim();
  const name = choice.name.trim();
  if (parent === "") return "Choose the folder to clone into";
  if (!isFullPath(parent)) return "Enter the full path of the parent folder";
  if (name === "") return "Enter a name for the new folder";
  if (FORBIDDEN_IN_NAME.test(name) || name === "." || name === "..") {
    return 'A folder name cannot contain \\ / : * ? " < > |';
  }
  const path = joinPath(parent, name);
  if (choice.seen?.path !== path) return "Checking the folder…";
  switch (choice.seen.kind) {
    case "notEmpty":
      return `${path} is not empty`;
    case "file":
      return `${path} is a file`;
    case "unreadable":
      return `${path} cannot be read`;
    default:
      return null;
  }
}

/** Where `path` is answered for, once it can be asked about at all. */
export function destinationToAsk(parent: string, name: string): string | null {
  const probe = directoryProblem({ parent, name, seen: null });
  return probe === "Checking the folder…" ? joinPath(parent.trim(), name.trim()) : null;
}

/** The server's default first and marked, then the rest in its order. */
export function branchOptions(listing: RemoteBranches): [string, string][] {
  const main = listing.defaultBranch;
  const listed = main !== null && listing.branches.includes(main);
  const rest = listing.branches.filter((branch) => !listed || branch !== main);
  const first: [string, string][] = listed ? [[main, `${main} (default)`]] : [];
  return [...first, ...rest.map((branch): [string, string] => [branch, branch])];
}

export function initialBranch(listing: RemoteBranches): string | null {
  const main = listing.defaultBranch;
  if (main !== null && listing.branches.includes(main)) return main;
  return listing.branches[0] ?? null;
}

export interface CloneChoices {
  source: string;
  submodules: boolean;
  allBranches: boolean;
  branch: string | null;
  /** `null` when the check was skipped: the server's HEAD decides. */
  listing: RemoteBranches | null;
  skipLarge: boolean;
  limitMb: string;
  parent: string;
  name: string;
}

/** `--branch` only for a branch other than the server's default: the command in Output then
    says what was asked for. */
export function cloneRequest(choices: CloneChoices): CloneRequest {
  const branch =
    choices.listing !== null && choices.branch !== null && choices.branch !== choices.listing.defaultBranch
      ? choices.branch
      : null;
  return {
    source: choices.source.trim(),
    target: joinPath(choices.parent.trim(), choices.name.trim()),
    submodules: choices.submodules,
    allBranches: choices.allBranches,
    branch,
    skipLargerThanMb: choices.skipLarge ? Number(choices.limitMb.trim()) : null,
  };
}

export interface CloneRun {
  /** The clone as a network operation: `network.run` with no repository behind it. */
  run: (operation: (onLine: (line: string) => void) => Promise<string>) => Promise<string>;
  clone: (request: CloneRequest, onLine: (line: string) => void) => Promise<string>;
  /** Opens the new repository and puts it in the list, as Open does. */
  open: (root: string) => Promise<unknown>;
  report: (err: unknown, title: string) => void;
}

/** The root it opened, or `null` when the clone failed or was cancelled. */
export async function runClone(request: CloneRequest, deps: CloneRun): Promise<string | null> {
  let root: string;
  try {
    root = await deps.run((onLine) => deps.clone(request, onLine));
  } catch (err) {
    deps.report(err, "Could not clone the repository");
    return null;
  }
  await deps.open(root);
  return root;
}
