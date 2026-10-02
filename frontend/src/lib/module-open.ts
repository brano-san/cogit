import { events } from "$lib/ipc/bindings";
import type { FileEntry, OpenModule, Submodule } from "$lib/ipc";
import { moduleKey, type ModuleRow } from "$lib/module-tree";
import { listenUntilUndone, type Emit, type Listen } from "$lib/settings-sync";

/** A compare window asked for a submodule: the main window opens it instead (R-537). */
export async function askToOpenModule(
  request: OpenModule,
  emit: Emit<OpenModule> = events.cogitOpenModule.emit,
): Promise<void> {
  await emit(request);
}

/** For the main window. Returns the undo, which also holds before the listener is in place. */
export function onOpenModule(
  handler: (request: OpenModule) => void,
  listen: Listen<OpenModule> = events.cogitOpenModule.listen,
): () => void {
  return listenUntilUndone(listen, handler);
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
