import { emit as tauriEmit, listen as tauriListen } from "@tauri-apps/api/event";
import type { FileEntry, RepoId, Submodule } from "$lib/ipc";
import { moduleKey, type ModuleRow } from "$lib/module-tree";
import type { Emit, Listen } from "$lib/settings-sync";

/** A compare window asked for a submodule: the main window opens it instead (R-537). Not
    declared in Rust, like `cogit://settings-changed` (R-518): only pages send and hear it. */
export const OPEN_MODULE = "cogit://open-module";

export interface ModuleRequest {
  repo: RepoId;
  path: string;
}

export async function askToOpenModule(request: ModuleRequest, emit: Emit = tauriEmit): Promise<void> {
  await emit(OPEN_MODULE, request);
}

function isRequest(payload: unknown): payload is ModuleRequest {
  const request = payload as Partial<ModuleRequest> | null;
  return typeof request?.repo === "number" && typeof request.path === "string";
}

/** For the main window. Returns the undo, which also holds before the listener is in place. */
export function onOpenModule(handler: (request: ModuleRequest) => void, listen: Listen = tauriListen): () => void {
  let stopped = false;
  let stop: (() => void) | null = null;
  void listen(OPEN_MODULE, (event) => {
    if (!stopped && isRequest(event.payload)) handler(event.payload);
  })
    .then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    })
    .catch(() => {});
  return () => {
    stopped = true;
    stop?.();
  };
}

/** Whether `path` is a submodule in any of the file lists on show: a gitlink has no lines
    to compare, so a double click opens it rather than a compare window. */
export function isModulePath(path: string, lists: readonly (readonly FileEntry[])[]): boolean {
  return lists.some((list) => list.some((entry) => entry.path === path && entry.mode === "submodule"));
}

/**
 * The Repositories row of the submodule at `path` of the repository the panels show: the
 * tree's owner when `open` is `null`, else the submodule `open` names. Its parent's list is
 * read when that node was never opened in the tree.
 */
export async function findModuleRow(
  children: ReadonlyMap<string, readonly Submodule[]>,
  open: string | null,
  path: string,
  list: (parent: string) => Promise<readonly Submodule[]>,
): Promise<ModuleRow | null> {
  const parent = open ?? "";
  const siblings = children.get(parent) ?? (await list(parent));
  const module = siblings.find((each) => each.path === path);
  if (!module) return null;
  return { key: moduleKey(parent, path), path, parent, depth: depthBelow(children, parent), module, expanded: false };
}

/** How deep a child of `parent` sits. A key holds the paths of every module above it, and a
    path has slashes of its own, so it is found by walking down from the top, not counted. */
function depthBelow(children: ReadonlyMap<string, readonly Submodule[]>, parent: string): number {
  if (parent === "") return 0;
  let level = [""];
  for (let depth = 1; level.length > 0; depth += 1) {
    const next: string[] = [];
    for (const key of level) {
      for (const each of children.get(key) ?? []) {
        const child = moduleKey(key, each.path);
        if (child === parent) return depth;
        next.push(child);
      }
    }
    level = next;
  }
  // A parent the tree never read: the submodule on show, opened from a list of its own.
  return 1;
}
