import { overlapWindow, type OverlapRow, type RepoId } from "$lib/ipc";
import { settings } from "$stores/settings.svelte";

class OverlapStore {
  /** The `graphOverlap` setting: the graph's options menu and Preferences both turn it. */
  get enabled(): boolean {
    return settings.current.graphOverlap;
  }
  rows = $state.raw<Map<string, OverlapRow>>(new Map());
  base = $state<string | null>(null);

  #generation = 0;

  toggle(): void {
    void settings.set("graphOverlap", !this.enabled);
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

  /** The row of `oid` against `selected`, the base the column is for: nothing on the
      Working Tree, and nothing while another commit's rows are on their way (F-093). */
  rowOf(oid: string, selected: string | null): OverlapRow | undefined {
    return selected !== null && selected === this.base ? this.rows.get(oid) : undefined;
  }

  clear(): void {
    this.#generation += 1;
    this.rows = new Map();
    this.base = null;
  }
}

export const overlap = new OverlapStore();
