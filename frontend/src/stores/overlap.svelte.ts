import { overlapWindow, type OverlapRow, type RepoId } from "$lib/ipc";

class OverlapStore {
  /** Off by default: the column costs a tree diff per visible row (M13). */
  enabled = $state(false);
  rows = $state.raw<Map<string, OverlapRow>>(new Map());
  base = $state<string | null>(null);

  #generation = 0;

  toggle(): void {
    this.enabled = !this.enabled;
    if (!this.enabled) this.clear();
  }

  async load(repo: RepoId, base: string, window: readonly string[]): Promise<void> {
    if (!this.enabled || window.length === 0) return;

    const fresh = base !== this.base;
    const missing = fresh ? window : window.filter((oid) => !this.rows.has(oid));
    if (missing.length === 0) return;

    const generation = ++this.#generation;
    try {
      const computed = await overlapWindow(repo, base, [...missing]);
      if (generation !== this.#generation) return;
      const next = fresh ? new Map<string, OverlapRow>() : new Map(this.rows);
      for (const row of computed) next.set(row.oid, row);
      this.rows = next;
      this.base = base;
    } catch {
      // A failed column is a blank column, not a dialog: it is an optional extra.
    }
  }

  clear(): void {
    this.#generation += 1;
    this.rows = new Map();
    this.base = null;
  }
}

export const overlap = new OverlapStore();
