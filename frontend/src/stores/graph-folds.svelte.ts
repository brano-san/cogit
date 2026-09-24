import type { RepoId } from "$lib/ipc";

/** Merges opened in the graph while merged branches fold (#26), until another repository
    is shown. */
export class GraphFolds {
  expanded = $state.raw<ReadonlySet<string>>(new Set());
  #repo: RepoId | null = null;

  forRepo(repo: RepoId | null): void {
    if (repo === this.#repo) return;
    this.#repo = repo;
    if (this.expanded.size > 0) this.expanded = new Set();
  }

  toggle(oid: string): void {
    const next = new Set(this.expanded);
    if (!next.delete(oid)) next.add(oid);
    this.expanded = next;
  }
}

export const graphFolds = new GraphFolds();
