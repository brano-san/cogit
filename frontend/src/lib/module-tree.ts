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

/** The word after the row's position, only where it is exact (R-153). `unknown` means the
    recorded commit is not in the submodule — that much is exact (R-179). */
const LABELS: Partial<Record<Submodule["state"], string>> = {
  ahead: "ahead",
  behind: "behind",
  diverged: "diverged",
  unknown: "not fetched",
};

/** Empty for a row of a light tree, which never looked inside the submodule (R-352). */
export function describeModule(module: Submodule): string {
  if (module.state === "notInitialised") return "not initialised";
  if (module.state === "unread") return "";
  const where =
    module.branch ??
    (module.checkedOut
      ? `${shortOid(module.checkedOut)}${module.subject ? `: ${module.subject}` : ""}`
      : "no commit checked out");
  const label = LABELS[module.state];
  return label ? `${where} · ${label}` : where;
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
        `${commits(module.behind)} older than the one the parent records. Run ` +
        "git submodule update (Update) to check out the recorded commit."
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
  }
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
