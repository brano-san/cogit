import type { Submodule } from "$lib/ipc";
import { submoduleOutline } from "$lib/ipc/repo-rows";
import { moduleRows, type ModuleRow } from "$lib/module-tree";
import { moduleMemory } from "$stores/module-memory.svelte";

type Children = ReadonlyMap<string, readonly Submodule[]>;

/** The submodule trees of every listed repository except the one the panels own: read by
    folder from `.gitmodules` and the gitlinks, so a closed repository has one too and
    nothing has to be activated to see it (R-352). */
class ModuleForest {
  trees = $state.raw<ReadonlyMap<string, Children>>(new Map());
  /** Roots whose top was read or is being read, so a re-render does not ask twice. */
  #asked = new Set<string>();
  /** Probes run one after another, however many callers ask at once. */
  #probing: Promise<void> = Promise.resolve();

  /** `undefined` until the top has been read. */
  hasModules(root: string): boolean | undefined {
    const top = this.trees.get(root)?.get("");
    return top === undefined ? undefined : top.length > 0;
  }

  rows(root: string): ModuleRow[] {
    if (!moduleMemory.isOpen(root)) return [];
    return moduleRows(this.trees.get(root) ?? new Map(), moduleMemory.expanded(root));
  }

  /** Reads the top of each root not read yet, one repository at a time; a tree left open
      at the last exit also gets its open nodes back. */
  probe(roots: readonly string[]): Promise<void> {
    const run = this.#probing.then(() => this.#probe(roots));
    this.#probing = run.catch(() => {});
    return run;
  }

  async #probe(roots: readonly string[]): Promise<void> {
    for (const root of roots) {
      if (this.#asked.has(root)) continue;
      this.#asked.add(root);
      await this.#load(root, "", false);
      if (moduleMemory.isOpen(root)) await this.#loadExpanded(root);
    }
  }

  /** Opening reads the top again: `.gitmodules` may have changed while it was folded. */
  async toggleTop(root: string): Promise<void> {
    const open = !moduleMemory.isOpen(root);
    moduleMemory.setOpen(root, open);
    if (!open) return;
    this.#asked.add(root);
    await this.#load(root, "", true);
    await this.#loadExpanded(root);
  }

  async toggle(root: string, row: ModuleRow): Promise<void> {
    const open = !row.expanded;
    moduleMemory.setNode(root, row.key, open);
    if (open) await this.#load(root, row.key, false);
  }

  /** What was read for `root` may be stale: the panels owned it and changed things. */
  forget(root: string): void {
    this.#asked.delete(root);
    // Written even when nothing was read: the list re-probes on every change of this.
    const trees = new Map(this.trees);
    trees.delete(root);
    this.trees = trees;
  }

  async #loadExpanded(root: string): Promise<void> {
    for (const key of moduleMemory.expanded(root)) await this.#load(root, key, false);
  }

  async #load(root: string, key: string, again: boolean): Promise<void> {
    if (!again && this.trees.get(root)?.has(key)) return;
    // A folder that is gone or not a repository has no tree; the row says why elsewhere.
    const found = await submoduleOutline(root, key).catch(() => []);
    const tree = new Map(this.trees.get(root) ?? []);
    tree.set(key, found);
    this.trees = new Map([...this.trees, [root, tree]]);
  }
}

export const moduleForest = new ModuleForest();
