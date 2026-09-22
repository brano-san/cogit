import { moduleRows, type ModuleRow } from "$lib/module-tree";
import { readKey, writeKey } from "$lib/settings-file";
import { listSubmodules, updateSubmodule, type RepoId, type Submodule } from "$lib/ipc";

const KEY = "submoduleTree";

class SubmoduleStore {
  /** Keyed by the path from the top repository down; `""` is the repository itself.
      Only the nodes somebody opened are in here: a repository with nine submodules,
      each with its own, would otherwise cost nine reads nobody asked for (R-110). */
  children = $state.raw<ReadonlyMap<string, Submodule[]>>(new Map());
  expanded = $state.raw<ReadonlySet<string>>(new Set());
  /** The submodule the panels are currently showing, by key; null for the repository. */
  open = $state<string | null>(null);

  /** The repository the tree belongs to. It is the one in the Repositories panel, which
      is not the same as the one the other panels are showing once a submodule has been
      opened from this very tree (R-109). */
  #repo: RepoId | null = null;
  #root: string | null = null;

  get entries(): readonly Submodule[] {
    return this.children.get("") ?? [];
  }

  get rows(): ModuleRow[] {
    return moduleRows(this.children, this.expanded);
  }

  /** Hands the tree to a repository. Called when one is activated from the list, and
      never when a submodule is opened from inside the tree. */
  async own(repo: RepoId, root: string): Promise<void> {
    this.#repo = repo;
    this.#root = root;
    this.open = null;
    this.children = new Map([["", await listSubmodules(repo, "").catch(() => [])]]);
    // What was open last time is opened again, one level at a time.
    const remembered = (await readKey<Record<string, string[]>>(KEY))?.[root] ?? [];
    this.expanded = new Set();
    for (const key of remembered) {
      await this.#load(key);
      this.expanded = new Set([...this.expanded, key]);
    }
  }

  async #remember(): Promise<void> {
    if (this.#root === null) return;
    const all = (await readKey<Record<string, string[]>>(KEY)) ?? {};
    await writeKey(KEY, { ...all, [this.#root]: [...this.expanded] });
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
      void this.#remember();
      return;
    }
    next.add(row.key);
    this.expanded = next;
    await this.#load(row.key);
    void this.#remember();
  }

  /** Reads one node's children, once. A node that turns out to have none stays known as
      empty rather than being asked again on every expand. */
  async #load(key: string): Promise<void> {
    if (this.#repo === null || this.children.has(key)) return;
    try {
      const found = await listSubmodules(this.#repo, key);
      this.children = new Map([...this.children, [key, found]]);
    } catch {
      // A module that is not initialised has no repository to read; an empty branch is
      // the honest answer and the row already says why.
      this.children = new Map([...this.children, [key, []]]);
    }
  }

  async update(repo: RepoId, path: string, init: boolean): Promise<void> {
    await updateSubmodule(repo, path, init);
    await this.refresh();
  }

  /** Puts the tree back after the panels were cleared for a submodule that was opened
      from it: the click came from this tree and it must not vanish underneath. */
  restore(
    children: ReadonlyMap<string, Submodule[]>,
    expanded: ReadonlySet<string>,
    open: string,
  ): void {
    this.children = children;
    this.expanded = expanded;
    this.open = open;
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
