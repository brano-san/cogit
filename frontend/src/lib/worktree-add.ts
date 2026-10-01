import { shortRev } from "$lib/format";
import type { Branch, WorktreeBranch } from "$lib/ipc";

export type AddMode = "new" | "existing" | "detached";

export interface AddForm {
  mode: AddMode;
  /** New branch: its name. */
  name: string;
  /** New branch and detached: where it starts (a revision as typed). */
  base: string;
  /** New branch from a remote branch: follow it. */
  track: boolean;
  /** Existing branch: a local or remote branch name. */
  existing: string;
  folder: string;
}

/** What the dialog was opened from: a commit in the graph, a branch in Branches, or neither. */
export type AddOrigin = { kind: "commit"; oid: string } | { kind: "branch"; name: string } | { kind: "current" };

export function defaultBase(origin: AddOrigin, currentBranch: string | null): string {
  if (origin.kind === "commit") return origin.oid;
  if (origin.kind === "branch") return origin.name;
  return currentBranch ?? "HEAD";
}

const slash = (path: string) => path.replace(/\\/g, "/").replace(/\/+$/, "");

/** `<folder next to the repository>/<repository>-<label>`, `/` and the characters a folder
    name cannot hold turned into `-`. */
export function suggestFolder(root: string, label: string): string {
  const slug = label
    .replace(/[\\/:*?"<>|~^\s]+/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/^-|-$/g, "");
  if (slug === "") return "";
  const clean = slash(root);
  return `${clean}-${slug}`;
}

/** `origin/feature/x` is `feature/x` locally. */
export function localNameOfRemote(name: string): string {
  return name.slice(name.indexOf("/") + 1);
}

/** The base when it is exactly a remote branch, else null. */
export function remoteBase(base: string, branches: readonly Branch[]): string | null {
  const name = base.trim();
  return branches.some((branch) => branch.kind === "remote" && branch.name === name) ? name : null;
}

const isRemote = (name: string, branches: readonly Branch[]) => remoteBase(name, branches) !== null;

/** What the folder name is made from. */
export function folderLabel(form: AddForm, branches: readonly Branch[]): string {
  if (form.mode === "new") return form.name.trim();
  if (form.mode === "existing") {
    return isRemote(form.existing, branches) ? localNameOfRemote(form.existing) : form.existing;
  }
  return shortRev(form.base.trim());
}

export function addRequest(form: AddForm, branches: readonly Branch[]): { path: string; branch: WorktreeBranch } {
  const start = form.base.trim() === "" ? null : form.base.trim();
  const path = slash(form.folder.trim());
  if (form.mode === "detached") return { path, branch: { kind: "detached", start } };
  if (form.mode === "new") {
    const track = start !== null && isRemote(start, branches) && form.track;
    return { path, branch: { kind: "new", name: form.name.trim(), start, track } };
  }
  if (isRemote(form.existing, branches)) {
    return {
      path,
      branch: { kind: "new", name: localNameOfRemote(form.existing), start: form.existing, track: true },
    };
  }
  return { path, branch: { kind: "existing", name: form.existing } };
}

function shown(folder: string, root: string): string {
  const dir = (path: string) => path.slice(0, path.lastIndexOf("/"));
  const path = slash(folder.trim());
  const sibling = path !== "" && dir(path) === dir(slash(root)) && dir(path) !== "";
  return sibling ? `../${path.slice(path.lastIndexOf("/") + 1)}` : path;
}

const quote = (arg: string) => (/\s/.test(arg) ? `"${arg}"` : arg);

/** The command the dialog will run, mirroring `WorktreeBranch::add_args` in git_engine. */
export function commandPreview(form: AddForm, branches: readonly Branch[], root: string): string {
  const { path, branch } = addRequest(form, branches);
  const where = shown(path, root) || "<folder>";
  const args = ["git", "worktree", "add"];
  if (branch.kind === "new") {
    if (branch.track) args.push("--track");
    else if (branch.start !== null && isRemote(branch.start, branches)) args.push("--no-track");
    args.push("-b", branch.name || "<name>", where);
    if (branch.start !== null) args.push(branch.start);
  } else if (branch.kind === "existing") {
    args.push(where, branch.name || "<branch>");
  } else {
    args.push("--detach", where);
    if (branch.start !== null) args.push(branch.start);
  }
  return args.map(quote).join(" ");
}

/** Answers from git, each tagged with the input it answers: one for older input is stale. */
export interface AddChecks {
  base: { rev: string; problem: string | null } | null;
  name: { name: string; problem: string | null } | null;
  folder: { path: string; problem: string | null } | null;
}

/** A new branch name that cannot be taken, from what is known without git and git's answer. */
export function newNameProblem(
  name: string,
  branches: readonly Branch[],
  answer: AddChecks["name"],
): string | null {
  if (branches.some((branch) => branch.kind === "local" && branch.name === name)) {
    return `A branch named ${name} already exists`;
  }
  return answer?.name === name ? answer.problem : null;
}

/** The first thing wrong, in the order of the form; `pending` while an answer is awaited. */
export function addProblem(
  form: AddForm,
  branches: readonly Branch[],
  checks: AddChecks,
  held: ReadonlyMap<string, string> = new Map(),
): { text: string | null; pending: boolean } {
  let pending = false;
  const wait = <T>(answer: T | null, current: boolean): T | null => {
    if (answer === null || !current) pending = true;
    return answer !== null && current ? answer : null;
  };

  if (form.mode === "new") {
    const name = form.name.trim();
    if (name === "") return { text: "Enter a name for the new branch", pending: false };
    const local = newNameProblem(name, branches, null);
    if (local) return { text: local, pending: false };
    const answer = wait(checks.name, checks.name?.name === name);
    if (answer?.problem) return { text: answer.problem, pending: false };
  } else if (form.mode === "existing") {
    const chosen = branches.find((branch) => branch.name === form.existing);
    if (!chosen) return { text: "Choose a branch", pending: false };
    const where = held.get(chosen.name);
    if (where !== undefined) return { text: `${chosen.name} is checked out in ${where}`, pending: false };
  }

  if (form.mode !== "existing") {
    const rev = form.base.trim();
    if (rev === "") return { text: "Choose where the branch starts", pending: false };
    const answer = wait(checks.base, checks.base?.rev === rev);
    if (answer?.problem) return { text: answer.problem, pending: false };
  }

  const path = slash(form.folder.trim());
  if (path === "") return { text: "Choose a folder for the worktree", pending: false };
  const answer = wait(checks.folder, checks.folder?.path === path);
  if (answer?.problem) return { text: answer.problem, pending: false };

  return { text: null, pending };
}

const OPEN_AFTER_KEY = "cogit.addWorktree.openAfter";

export function rememberedOpenAfter(): boolean {
  try {
    return localStorage.getItem(OPEN_AFTER_KEY) === "1";
  } catch {
    return false;
  }
}

export function rememberOpenAfter(on: boolean): void {
  try {
    localStorage.setItem(OPEN_AFTER_KEY, on ? "1" : "0");
  } catch {
    // Not remembered; the box starts off next time.
  }
}
