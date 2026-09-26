import { shortOid } from "$lib/format";
import type { Submodule } from "$lib/ipc";

/** A submodule as a row of the repository tree: its own path, how deep it sits, and
    whether its own submodules are showing. SmartGit draws these as branches of the
    repository they belong to, not as a flat list beside it (doc/12-risks.md, R-110). */
export interface ModuleRow {
  /** The path from the top repository down, which is what keeps a node expanded across
      a refresh however the rows are ordered. */
  key: string;
  /** The path inside the repository that owns it, which is what the backend asks for. */
  path: string;
  parent: string;
  depth: number;
  module: Submodule;
  expanded: boolean;
}

const MAX_DEPTH = 16;

export function splitModulePath(path: string): { dir: string; name: string } {
  const trimmed = path.replace(/[/\\]+$/, "");
  const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  return cut < 0
    ? { dir: "", name: trimmed }
    : { dir: trimmed.slice(0, cut + 1), name: trimmed.slice(cut + 1) };
}

export function moduleKey(parent: string, path: string): string {
  return parent === "" ? path : `${parent}/${path}`;
}

/** A node's folder: its key is the path from the top repository down (R-149). */
export function moduleRoot(top: string, key: string): string {
  return `${top.replace(/[/\\]+$/, "")}/${key}`;
}

/** The folders whose pulse gives the nodes their marks; one not checked out has no
    repository to read (R-542). */
export function pulsedRoots(top: string, rows: readonly ModuleRow[]): string[] {
  return rows.filter((row) => row.module.state !== "notInitialised").map((row) => moduleRoot(top, row.key));
}

export interface ShownPanels {
  current: string | null;
  moduleOwnerRoot: string | null;
  /** Key of the submodule the panels show, from the tree's owner. */
  openModule: string | null;
  /** Owner of the worktree the panels show, which may be the submodule itself. */
  worktreeOwnerRoot: string | null;
}

/** The row the panels show, named the way the list names it: a submodule by its node, not
    by the root the backend spells its own way, or leaving it would read no pulse for it. */
export function shownRowRoot(panels: ShownPanels): string | null {
  const { current, moduleOwnerRoot, openModule, worktreeOwnerRoot } = panels;
  if (current !== null && openModule !== null && moduleOwnerRoot !== null && worktreeOwnerRoot === null) {
    return moduleRoot(moduleOwnerRoot, openModule);
  }
  return current;
}

/** The word for the row's position, only where it is exact (R-153). `unknown` means the
    recorded commit is not in the submodule — that much is exact (R-179). */
const LABELS: Partial<Record<Submodule["state"], string>> = {
  notInitialised: "not initialized",
  ahead: "ahead",
  behind: "behind",
  diverged: "diverged",
  unknown: "not fetched",
  unrecorded: "not recorded",
};

export interface ModuleHint {
  /** Never cut: the row shows it whole, before the text it cuts (R-544). */
  label: string | null;
  /** The branch, or the commit and its subject; the part of the row that gives way. */
  where: string;
}

/** Nothing for a row of a light tree, which never looked inside the submodule (R-352). */
export function moduleHint(module: Submodule): ModuleHint {
  const label = LABELS[module.state] ?? null;
  if (module.state === "notInitialised" || module.state === "unread") return { label, where: "" };
  const where =
    module.branch ??
    (module.checkedOut
      ? `${shortOid(module.checkedOut)}${module.subject ? `: ${module.subject}` : ""}`
      : "no commit checked out");
  return { label, where };
}

function commits(count: number): string {
  return count === 1 ? "1 commit" : `${count} commits`;
}

export function moduleTooltip(module: Submodule): string {
  switch (module.state) {
    case "inSync":
    case "unread":
      return "";
    case "notInitialised":
      return "Not checked out yet. Initialise it to get its files.";
    case "ahead":
      return (
        `${commits(module.ahead)} newer than the one the parent records. Commit the new ` +
        "submodule pointer in the parent to keep them."
      );
    case "behind":
      return (
        `${commits(module.behind)} older than the one the parent records. ` +
        "Update in its menu runs git submodule update to check out the recorded commit."
      );
    case "diverged":
      return (
        `${commits(module.ahead)} of its own and ${commits(module.behind)} of the parent's ` +
        "that it lacks. Neither contains the other, so someone has to decide by hand which " +
        "one the parent should record."
      );
    case "unknown":
      return (
        "Not fetched: the parent records a commit this submodule does not have, so it " +
        "cannot be compared with it. Fetch in the submodule."
      );
    case "unrecorded":
      return (
        "Not recorded: .gitmodules lists it, but neither HEAD nor the index holds a commit " +
        "for it. Stage it in the parent to record the commit it is on."
      );
  }
}

export interface UpdateQuestion {
  title: string;
  message: string;
  confirm: string;
  warning: boolean;
}

/** Update in a submodule row's menu (F-521). `git submodule update` checks out the commit
    the parent records and detaches HEAD there, so a module with commits of its own is
    asked about first; one that is only behind loses nothing. */
export type ModuleUpdate =
  | { kind: "run"; init: boolean }
  | { kind: "ask"; request: UpdateQuestion }
  | { kind: "off"; reason: string };

export function moduleUpdate(module: Submodule): ModuleUpdate {
  switch (module.state) {
    case "notInitialised":
      return { kind: "run", init: true };
    case "behind":
      return { kind: "run", init: false };
    case "ahead":
    case "diverged": {
      const kept = module.branch
        ? `they stay on ${module.branch}`
        : "they are then on no branch, and only its reflog keeps them";
      return {
        kind: "ask",
        request: {
          title: "Update Submodule",
          message:
            `${module.path} has ${commits(module.ahead)} the parent does not record. Update ` +
            `checks out the recorded commit and detaches HEAD there; ${kept}.`,
          confirm: "Update",
          warning: true,
        },
      };
    }
    case "inSync":
      return { kind: "off", reason: "on the recorded commit" };
    case "unknown":
      return { kind: "off", reason: "recorded commit not fetched" };
    case "unread":
      return { kind: "off", reason: "open its repository first" };
    case "unrecorded":
      return { kind: "off", reason: "the parent records no commit" };
  }
}

export interface UpdateDeps {
  ask: (request: UpdateQuestion) => Promise<boolean>;
  update: (init: boolean) => Promise<void>;
}

/** Whether the update ran. */
export async function updateModule(module: Submodule, deps: UpdateDeps): Promise<boolean> {
  const plan = moduleUpdate(module);
  if (plan.kind === "off") return false;
  if (plan.kind === "ask" && !(await deps.ask(plan.request))) return false;
  await deps.update(plan.kind === "run" && plan.init);
  return true;
}

export function moduleRows(
  children: ReadonlyMap<string, readonly Submodule[]>,
  expanded: ReadonlySet<string>,
): ModuleRow[] {
  const rows: ModuleRow[] = [];

  const walk = (parent: string, depth: number) => {
    if (depth > MAX_DEPTH) return;
    for (const module of children.get(parent) ?? []) {
      const key = moduleKey(parent, module.path);
      const open = expanded.has(key);
      rows.push({ key, path: module.path, parent, depth, module, expanded: open });
      if (open) walk(key, depth + 1);
    }
  };

  walk("", 0);
  return rows;
}

/** Does this node deserve a caret? Before anything has been read, the answer is the
    one the backend already gave: `nested` is a file test it did while listing the row.
    A look that found nothing overrules it (R-123, R-148). */
export function mayExpand(
  children: ReadonlyMap<string, readonly Submodule[]>,
  key: string,
  module: Submodule,
): boolean {
  const found = children.get(key);
  return found === undefined ? module.nested : found.length > 0;
}
