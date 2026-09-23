import type { ContextItem } from "./ipc";
import type { DesktopInfo } from "./ipc/file-menus";
import { SEPARATOR, dropStraySeparators, entry, explained, submenu } from "./menu-entries";
import { UNGROUPED, type RepoGroups } from "./repo-groups";

export const REPO_MOVE_PREFIX = "repo-move:";
export const REPO_MOVE_NEW = "repo-move-new";

/** Shown on the four list-only items of a submodule node: it is part of its parent. */
export const SUBMODULE_REASON = "a submodule belongs to its parent";

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
  pinned: boolean;
  group: string;
  groups: readonly GroupChoice[];
}

export function groupChoices(groups: RepoGroups): GroupChoice[] {
  const children = new Map<string | null, string[]>();
  for (const id of groups.order) {
    const parent = groups.under[id] ?? null;
    children.set(parent, [...(children.get(parent) ?? []), id]);
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
  if (at.kind === "submodule") return explained("repo-move", "Move To", SUBMODULE_REASON);
  return submenu("repo-move", "Move To", [
    entry(REPO_MOVE_PREFIX, "No Group", at.group !== UNGROUPED),
    ...at.groups.map((group) =>
      entry(
        `${REPO_MOVE_PREFIX}${group.id}`,
        `${"    ".repeat(group.depth)}${group.name}`,
        group.id !== at.group,
      ),
    ),
    SEPARATOR,
    entry(REPO_MOVE_NEW, "New Group…"),
  ]);
}

/** SmartGit's order (#36). PowerShell and Git Shell exist only on Windows and are left
    out elsewhere; everything else is disabled, not hidden, when it does not apply. */
export function repoMenu(at: RepoMenuTarget, desktop: DesktopInfo): ContextItem[] {
  const gone = at.missing ? "the folder is missing" : null;
  const closed = at.open ? gone : "closed";
  const listOnly = at.kind === "submodule" ? SUBMODULE_REASON : null;
  const onlyActive = (chord: string) => (at.active ? chord : undefined);
  const manager = desktop.fileManager;

  return dropStraySeparators([
    entry("repo-open", "Open Repository", !at.active),
    explained("repo-open-folder", `Open in ${manager}`, gone),
    explained("repo-reveal", `Reveal in ${manager}`, gone),
    explained("repo-terminal", "Open in Terminal", gone),
    ...(desktop.windowsShells
      ? [
          explained("repo-powershell", "Open in PowerShell", gone),
          explained(
            "repo-git-shell",
            "Open in Git Shell",
            gone ?? (desktop.gitShell === null ? "Git Bash not found" : null),
          ),
        ]
      : []),
    entry("repo-close", "Close Repository", at.open, onlyActive("CmdOrCtrl+W")),
    SEPARATOR,
    explained("repo-pull", "Pull", closed, onlyActive("CmdOrCtrl+Shift+U")),
    explained("repo-push", "Push", closed, onlyActive("CmdOrCtrl+Shift+O")),
    SEPARATOR,
    moveTo(at),
    explained("repo-pin", at.pinned ? "Unpin" : "Pin", listOnly),
    explained("repo-rename", "Rename…", listOnly),
    explained("repo-remove", "Remove…", listOnly),
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
