import { moduleRows, type ModuleRow } from "$lib/module-tree";
import { recall, remember } from "$lib/session-memory";
import { listSubmodules, updateSubmodule, type RepoId, type Submodule } from "$lib/ipc";


class SubmoduleStore {
  /** Keyed by the path from the top repository down; `""` is the repository itself.
      Only the nodes somebody opened are in here: a repository with nine submodules,
      each with its own, would otherwise cost nine reads nobody asked for (R-110). */
  children = $state.raw<ReadonlyMap<string, Submodule[]>>(new Map());
  expanded = $state.raw<ReadonlySet<string>>(new Set());
  /** The submodule the panels are currently showing, by key; null for the repository. */
  open = $state<string | null>(null);
  folded = $state(true);

  get top(): readonly Submodule[] {
    return this.children.get("") ?? [];
  }

  foldTop(): void {
    this.folded = !this.folded;
    const root = this.#root;
    if (root === null) return;
    const open = new Set(recall("submodules-top", ""));
    if (this.folded) open.delete(root);
    else open.add(root);
    remember("submodules-top", "", open);
  }

  #repo = $state.raw<RepoId | null>(null);
  #root: string | null = null;

  /** The repository whose tree this is — the one in the list. Not the same as the one
      the other panels are showing once a submodule has been opened from it (R-129). */
  get owner(): RepoId | null {
    return this.#repo;
  }

  get ownerRoot(): string | null {
    return this.#root;
  }

  get entries(): readonly Submodule[] {
    return this.children.get("") ?? [];
  }

  get rows(): ModuleRow[] {
    return this.folded ? [] : moduleRows(this.children, this.expanded);
  }

  /** Hands the tree to a repository. Called when one is activated from the list, and
      never when a submodule is opened from inside the tree. */
  async own(repo: RepoId, root: string): Promise<void> {
    this.#repo = repo;
    this.#root = root;
    this.open = null;
    this.children = new Map([["", await listSubmodules(repo, "").catch(() => [])]]);
    this.folded = !recall("submodules-top", "").has(root);
    this.expanded = new Set();
    for (const key of recall("submodules", root)) {
      await this.#load(key);
      this.expanded = new Set([...this.expanded, key]);
    }
  }

  #remember(): void {
    if (this.#root !== null) remember("submodules", this.#root, this.expanded);
  }

  /** Re-reads what is on screen. Every mutation lands here, so collapsing the tree each
      time would mean the tree is only ever open between two commits. */
  async refresh(): Promise<void> {
    const repo = this.#repo;
    if (repo === null) return;
    const read = await Promise.all(
      ["", ...this.expanded].map(
        async (key) => [key, await listSubmodules(repo, key).catch(() => [])] as const,
      ),
    );
    this.children = new Map(read);
  }

  async toggle(row: ModuleRow): Promise<void> {
    const next = new Set(this.expanded);
    if (next.has(row.key)) {
      next.delete(row.key);
      this.expanded = next;
      this.#remember();
      return;
    }
    next.add(row.key);
    this.expanded = next;
    await this.#load(row.key);
    this.#remember();
  }

  /** Reads one node's children, once. A node that turns out to have none stays known as
      empty rather than being asked again on every expand. */
  async #load(key: string): Promise<void> {
    if (this.#repo === null || this.children.has(key)) return;
    try {
      const found = await listSubmodules(this.#repo, key);
      this.children = new Map([...this.children, [key, found]]);
    } catch {
      // Not initialised: nothing to read, and the row already says why.
      this.children = new Map([...this.children, [key, []]]);
    }
  }

  async update(repo: RepoId, path: string, init: boolean): Promise<void> {
    await updateSubmodule(repo, path, init);
    await this.refresh();
  }

  clear(): void {
    this.children = new Map();
    this.expanded = new Set();
    this.open = null;
    this.#repo = null;
    this.#root = null;
  }
}

export const submodules = new SubmoduleStore();
