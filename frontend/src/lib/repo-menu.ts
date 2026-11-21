import type { Keymap } from "./keymap";
import { revealLabel } from "./platform";
import type { ContextItem, Submodule } from "./ipc";
import type { DesktopInfo } from "./ipc/file-menus";
import { SEPARATOR, item, offer, submenu, tidy } from "./context-menu";
import { moduleUpdate } from "./module-tree";
import { UNGROUPED, type RepoGroups } from "./repo-groups";

export const REPO_MOVE_PREFIX = "repo-move:";
export const REPO_MOVE_NEW = "repo-move-new";

/** Shown on the four list-only items of a submodule node: it is part of its parent. */
export const SUBMODULE_REASON = "a submodule belongs to its parent";

/** Shown on Update of a repository row: only a submodule has a commit recorded for it. */
export const NOT_A_SUBMODULE = "not a submodule";

export interface GroupChoice {
  id: string;
  name: string;
  depth: number;
}

export interface RepoMenuTarget {
  kind: "repository" | "submodule";
  active: boolean;
  /** Loaded, and so watched; a closed row is only remembered by the list. */
  open: boolean;
  missing: boolean;
  desktopBlock?: string | null;
  pinned: boolean;
  group: string;
  groups: readonly GroupChoice[];
  /** The node's submodule, which decides whether Update applies (F-521). */
  module?: Submodule;
}

function updateReason(at: RepoMenuTarget): string | null {
  if (at.kind !== "submodule") return NOT_A_SUBMODULE;
  const plan = at.module ? moduleUpdate(at.module) : null;
  if (!plan) return "open its repository first";
  return plan.kind === "off" ? plan.reason : null;
}

export function groupChoices(groups: RepoGroups): GroupChoice[] {
  const children = new Map<string | null, string[]>();
  for (const id of groups.order) {
    const parent = groups.under[id] ?? null;
    const siblings = children.get(parent);
    if (siblings) siblings.push(id);
    else children.set(parent, [id]);
  }
  const out: GroupChoice[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const id of children.get(parent) ?? []) {
      out.push({ id, name: groups.names[id] ?? id, depth });
      walk(id, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

function moveTo(at: RepoMenuTarget): ContextItem {
  if (at.kind === "submodule") return offer("repo-move", "Move To", SUBMODULE_REASON);
  return submenu("repo-move", "Move To", [
    item(REPO_MOVE_PREFIX, "No Group", at.group !== UNGROUPED),
    ...at.groups.map((group) =>
      item(
        `${REPO_MOVE_PREFIX}${group.id}`,
        `${"    ".repeat(group.depth)}${group.name}`,
        group.id !== at.group,
      ),
    ),
    SEPARATOR,
    item(REPO_MOVE_NEW, "New Group…"),
  ]);
}

/** A fixed order (#36). PowerShell and Git Shell exist only on Windows and are left
    out elsewhere; everything else is disabled, not hidden, when it does not apply. */
export function repoMenu(at: RepoMenuTarget, desktop: DesktopInfo, keys: Keymap): ContextItem[] {
  const gone = at.missing ? "the folder is missing" : null;
  const local = at.desktopBlock ?? gone;
  const closed = at.open ? gone : "closed";
  const listOnly = at.kind === "submodule" ? SUBMODULE_REASON : null;
  const onlyActive = (id: string) => (at.active ? keys[id] || undefined : undefined);
  const manager = desktop.fileManager;

  return tidy([
    at.active ? item("repo-open", "Open Repository", false) : offer("repo-open", "Open Repository", gone),
    offer("repo-open-folder", `Open in ${manager}`, local),
    offer("repo-reveal", revealLabel(manager), local),
    offer("repo-terminal", "Open in Terminal", local),
    ...(desktop.windowsShells
      ? [
          offer("repo-powershell", "Open in PowerShell", local),
          offer(
            "repo-git-shell",
            "Open in Git Shell",
            local ?? (desktop.gitShell === null ? "Git Bash not found" : null),
          ),
        ]
      : []),
    item("repo-close", "Close Repository", at.open, onlyActive("close")),
    SEPARATOR,
    offer("repo-pull", "Pull", closed, onlyActive("pull")),
    offer("repo-push", "Push", closed, onlyActive("push")),
    offer("repo-update", "Update", updateReason(at)),
    offer("repo-edit-ignore", "Edit .gitignore…", closed),
    SEPARATOR,
    moveTo(at),
    offer("repo-pin", at.pinned ? "Unpin" : "Pin", listOnly),
    offer("repo-rename", "Rename…", listOnly),
    offer("repo-remove", "Remove…", listOnly),
  ]);
}

export type RepoCommand =
  | { kind: "move"; group: string }
  | { kind: "move-new" }
  | { kind: "plain"; id: string };

export function parseRepoCommand(id: string): RepoCommand | null {
  if (id === REPO_MOVE_NEW) return { kind: "move-new" };
  if (id.startsWith(REPO_MOVE_PREFIX)) return { kind: "move", group: id.slice(REPO_MOVE_PREFIX.length) };
  return id.startsWith("repo-") ? { kind: "plain", id } : null;
}
