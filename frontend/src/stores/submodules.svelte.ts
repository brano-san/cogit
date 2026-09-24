import { moduleRows, type ModuleRow } from "$lib/module-tree";
import { listSubmodules, updateSubmodule, type RepoId, type Submodule } from "$lib/ipc";
import { moduleForest } from "$stores/module-forest.svelte";
import { moduleMemory } from "$stores/module-memory.svelte";

class SubmoduleStore {
  /** Keyed by the path from the top repository down; `""` is the repository itself.
      Only the nodes somebody opened are in here: a repository with nine submodules,
      each with its own, would otherwise cost nine reads nobody asked for (R-110). */
  children = $state.raw<ReadonlyMap<string, Submodule[]>>(new Map());
  /** The submodule the panels are currently showing, by key; null for the repository. */
  open = $state<string | null>(null);

  /** Open nodes and the fold of the top live in the memory every tree of the list shares,
      which is what keeps them across a restart (R-352). */
  get expanded(): ReadonlySet<string> {
    return this.#root === null ? new Set() : moduleMemory.expanded(this.#root);
  }

  get folded(): boolean {
    return this.#root === null || !moduleMemory.isOpen(this.#root);
  }

  get top(): readonly Submodule[] {
    return this.children.get("") ?? [];
  }

  foldTop(): void {
    if (this.#root !== null) moduleMemory.setOpen(this.#root, this.folded);
  }

  #repo = $state.raw<RepoId | null>(null);
  #root = $state<string | null>(null);
  /** Changes whenever the tree changes hands; a read begun for the previous owner must
      not land in the new one's tree. */
  #generation = 0;
  /** Only the newest re-read writes the tree. */
  #refreshes = 0;

  /** The repository whose tree this is — the one in the list. Not the same as the one
      the other panels are showing once a submodule has been opened from it (R-129). */
  get owner(): RepoId | null {
    return this.#repo;
  }

  get ownerRoot(): string | null {
    return this.#root;
  }

  get rows(): ModuleRow[] {
    return this.folded ? [] : moduleRows(this.children, this.expanded);
  }

  /** Hands the tree to a repository. Called when one is activated from the list, and
      never when a submodule is opened from inside the tree. */
  async own(repo: RepoId, root: string): Promise<void> {
    const generation = ++this.#generation;
    this.#handOver(root);
    this.#repo = repo;
    this.#root = root;
    this.open = null;
    this.children = new Map();
    const top = await listSubmodules(repo, "").catch(() => []);
    if (generation !== this.#generation) return;
    this.children = new Map([["", top]]);
    for (const key of moduleMemory.expanded(root)) {
      await this.#load(key);
      if (generation !== this.#generation) return;
    }
  }

  /** Re-reads what is on screen. Every mutation lands here, so collapsing the tree each
      time would mean the tree is only ever open between two commits. */
  async refresh(): Promise<void> {
    const repo = this.#repo;
    if (repo === null) return;
    const generation = this.#generation;
    const ticket = ++this.#refreshes;
    const read = new Map(
      await Promise.all(
        ["", ...this.expanded].map(
          async (key) => [key, await listSubmodules(repo, key).catch(() => [])] as const,
        ),
      ),
    );
    if (generation !== this.#generation || ticket !== this.#refreshes) return;
    // A node opened while this read was on its way is not in it; its own read is newer.
    for (const key of this.expanded) {
      const opened = this.children.get(key);
      if (!read.has(key) && opened) read.set(key, opened);
    }
    this.children = read;
  }

  async toggle(row: ModuleRow): Promise<void> {
    const root = this.#root;
    if (root === null) return;
    const open = !this.expanded.has(row.key);
    moduleMemory.setNode(root, row.key, open);
    if (open) await this.#load(row.key);
  }

  /** Reads one node's children, once. A node that turns out to have none stays known as
      empty rather than being asked again on every expand. */
  async #load(key: string): Promise<void> {
    if (this.#repo === null || this.children.has(key)) return;
    const generation = this.#generation;
    // Not initialised: nothing to read, and the row already says why.
    const found = await listSubmodules(this.#repo, key).catch(() => []);
    if (generation === this.#generation) this.children = new Map([...this.children, [key, found]]);
  }

  async update(repo: RepoId, path: string, init: boolean): Promise<void> {
    await updateSubmodule(repo, path, init);
    await this.refresh();
  }

  /** The light tree read before the panels owned a repository may be stale by the time
      they let go of it. */
  #handOver(next: string | null): void {
    if (this.#root !== null && this.#root !== next) moduleForest.forget(this.#root);
  }

  clear(): void {
    this.#handOver(null);
    this.#generation += 1;
    this.children = new Map();
    this.open = null;
    this.#repo = null;
    this.#root = null;
  }
}

export const submodules = new SubmoduleStore();
