import type { PaletteCommand } from "./palette";
import type { Submodule } from "./ipc";
import { moduleKey } from "./module-tree";

/** Remote ▸ Synchronise, Submodule, Subtree and LFS, and Repository ▸ Settings (#42, #45,
    #46): what each is called, when it is offered and why not. Ids are the native menu's. */

export type SubmoduleAction =
  | "initialize"
  | "synchronize"
  | "reset"
  | "add"
  | "deactivate"
  | "deinit"
  | "unregister";
export type SubtreeAction = "add" | "merge" | "split" | "reset" | "push";
export type LfsAction = "install" | "track" | "lock" | "unlock" | "prune";

export interface RemoteMenuContext {
  repository: boolean;
  remote: boolean;
  /** Anything uncommitted: `git subtree add` and `merge` refuse to start then. */
  changes: boolean;
  submodules: number;
  /** `undefined` while it is still being asked, `null` when git has no `lfs` command. */
  lfs: string | null | undefined;
  files: readonly string[];
  /** Why Synchronise cannot run with a remote there: the toolbar's Sync rule (`reasonOf`),
      which wants HEAD on a branch that tracks one. */
  syncBlocked?: string;
}

export interface RemoteMenuActions {
  synchronize: () => void;
  submodule: (action: SubmoduleAction) => void;
  subtree: (action: SubtreeAction) => void;
  lfs: (action: LfsAction) => void;
  repoSettings: () => void;
}

const SUBMODULE_ITEMS: readonly [string, SubmoduleAction, string][] = [
  ["submodule-init", "initialize", "Initialise"],
  ["submodule-sync", "synchronize", "Synchronise"],
  ["submodule-reset", "reset", "Reset…"],
  ["submodule-add", "add", "Add…"],
  ["submodule-deactivate", "deactivate", "Deactivate…"],
  ["submodule-deinit", "deinit", "Deinit…"],
  ["submodule-unregister", "unregister", "Unregister…"],
];

const SUBTREE_ITEMS: readonly [string, SubtreeAction, string][] = [
  ["subtree-add", "add", "Add…"],
  ["subtree-merge", "merge", "Merge…"],
  ["subtree-split", "split", "Split…"],
  ["subtree-reset", "reset", "Reset…"],
  ["subtree-push", "push", "Push…"],
];

const LFS_ITEMS: readonly [string, LfsAction, string][] = [
  ["lfs-install", "install", "Install"],
  ["lfs-track", "track", "Track…"],
  ["lfs-lock", "lock", "Lock"],
  ["lfs-unlock", "unlock", "Unlock"],
  ["lfs-prune", "prune", "Prune…"],
];

export const NO_REPOSITORY = "No repository is open";
export const NO_SUBMODULES = "This repository has no submodules";
export const UNCOMMITTED = "Commit or stash your changes first: git subtree needs a clean working tree";
export const NO_LFS = "Git LFS is not installed";
export const LFS_UNKNOWN = "Checking whether Git LFS is installed";
export const NO_FILE = "Select a file in the Files panel";

function submoduleReason(action: SubmoduleAction, context: RemoteMenuContext): string | undefined {
  if (!context.repository) return NO_REPOSITORY;
  if (action === "add") return undefined;
  return context.submodules > 0 ? undefined : NO_SUBMODULES;
}

function subtreeReason(action: SubtreeAction, context: RemoteMenuContext): string | undefined {
  if (!context.repository) return NO_REPOSITORY;
  const needsClean = action === "add" || action === "merge";
  return needsClean && context.changes ? UNCOMMITTED : undefined;
}

/** Only Install stays available without the extension: it explains how to get it. */
function lfsReason(action: LfsAction, context: RemoteMenuContext): string | undefined {
  if (!context.repository) return NO_REPOSITORY;
  if (action === "install") return undefined;
  if (context.lfs === undefined) return LFS_UNKNOWN;
  if (context.lfs === null) return NO_LFS;
  const needsFile = action === "lock" || action === "unlock";
  return needsFile && context.files.length === 0 ? NO_FILE : undefined;
}

export function remoteCommands(
  context: RemoteMenuContext,
  actions: RemoteMenuActions,
): PaletteCommand[] {
  return [
    {
      id: "synchronize",
      title: "Synchronise",
      synonyms: ["sync", "pull then push"],
      unavailable: !context.repository
        ? NO_REPOSITORY
        : context.remote
          ? context.syncBlocked
          : "This repository has no remote",
      run: actions.synchronize,
    },
    ...SUBMODULE_ITEMS.map(([id, action, label]) => ({
      id,
      title: `Submodule: ${label}`,
      synonyms: ["submodule", `git submodule ${action}`],
      unavailable: submoduleReason(action, context),
      run: () => actions.submodule(action),
    })),
    ...SUBTREE_ITEMS.map(([id, action, label]) => ({
      id,
      title: `Subtree: ${label}`,
      synonyms: ["subtree", `git subtree ${action}`],
      unavailable: subtreeReason(action, context),
      run: () => actions.subtree(action),
    })),
    ...LFS_ITEMS.map(([id, action, label]) => ({
      id,
      title: `LFS: ${label}`,
      synonyms: ["lfs", "large files", `git lfs ${action}`],
      unavailable: lfsReason(action, context),
      run: () => actions.lfs(action),
    })),
    {
      id: "repo-settings",
      title: "Repository Settings…",
      synonyms: ["user.name", "email", "pull rebase", "push default", "signing", "encoding", "tag grouping"],
      unavailable: context.repository ? undefined : NO_REPOSITORY,
      run: actions.repoSettings,
    },
  ];
}

/** Which repository's submodules an action is about, and which one the user pointed at. */
export interface SubmoduleScope {
  /** Key of the owning repository in the submodule tree; `""` is the top repository. */
  parent: string;
  choices: string[];
  path: string | null;
}

/** A submodule opened in the panels is acted on in the repository that holds it;
    otherwise the top repository, with a submodule picked in the Files panel if any. */
export function submoduleScope(input: {
  children: ReadonlyMap<string, readonly Submodule[]>;
  open: string | null;
  selected: readonly string[];
}): SubmoduleScope {
  if (input.open !== null) {
    for (const [parent, modules] of input.children) {
      const found = modules.find((module) => moduleKey(parent, module.path) === input.open);
      if (found) return { parent, choices: modules.map((module) => module.path), path: found.path };
    }
  }
  const choices = (input.children.get("") ?? []).map((module) => module.path);
  return { parent: "", choices, path: input.selected.find((path) => choices.includes(path)) ?? null };
}

/** `*.psd` for `art/cover.psd`: what Track usually wants for the file at hand. */
export function trackSuggestion(files: readonly string[]): string {
  const name = files[0]?.split("/").at(-1) ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? `*${name.slice(dot)}` : "";
}
