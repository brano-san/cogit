import {
  CogitError,
  EMPTY_QUERY,
  loadCommits,
  type CommitQuery,
  type CommitRow,
  type GraphRow,
  type RepoId,
} from "$lib/ipc";

export interface GraphEntry {
  commit: CommitRow;
  layout: GraphRow;
}

class GraphStore {
  /** `$state.raw`: 50 000 commits would otherwise become 50 000 reactive proxies. */
  rows = $state.raw<GraphEntry[]>([]);
  loading = $state(false);
  complete = $state(false);
  error = $state<CogitError | null>(null);
  skipped = $state.raw<import("$lib/ipc").SkippedRef[]>([]);

  /** Discriminates concurrent loads; chunks from a superseded stream are dropped. */
  #generation = 0;

  query = $state.raw<CommitQuery>(EMPTY_QUERY);

  /** `null` is every ref. Owned by the References panel, folded into every load. */
  visibleRefs = $state.raw<string[] | null>(null);

  /** Asking twice for the same commit has to scroll twice, hence the counter. */
  reveal = $state.raw<{ oid: string; request: number } | null>(null);

  requestReveal(oid: string): void {
    this.reveal = { oid, request: (this.reveal?.request ?? 0) + 1 };
  }

  #repo: RepoId | null = null;

  async load(repo: RepoId, query: CommitQuery = EMPTY_QUERY): Promise<void> {
    const generation = ++this.#generation;
    this.query = query;
    // Old rows stay until the new walk catches up: an empty list between flashed (R-186).
    const onScreen = this.#repo === repo ? this.rows.length : 0;
    if (onScreen === 0) this.rows = [];
    this.#repo = repo;
    this.error = null;
    this.skipped = [];
    this.complete = false;
    this.loading = true;
    let pending: GraphEntry[] | null = onScreen > 0 ? [] : null;

    try {
      const skipped = await loadCommits(repo, (chunk) => {
        if (generation !== this.#generation) return;

        const incoming = chunk.commits.map((commit, index) => ({
          commit,
          layout: chunk.rows[index]!,
        }));
        if (pending) {
          pending.push(...incoming);
          if (pending.length >= onScreen || chunk.isLast) {
            this.rows = pending;
            pending = null;
          }
        } else if (incoming.length > 0) {
          this.rows = [...this.rows, ...incoming];
        }
        if (chunk.isLast) this.complete = true;
      }, { ...query, visibleRefs: this.visibleRefs });
      if (generation === this.#generation) {
        if (pending) this.rows = pending;
        this.skipped = skipped ?? [];
      }
    } catch (err) {
      if (generation === this.#generation) {
        this.rows = [];
        this.error =
          err instanceof CogitError
            ? err
            : new CogitError({ kind: "internal", data: String(err) });
      }
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  clear(): void {
    this.#generation += 1;
    this.query = EMPTY_QUERY;
    this.visibleRefs = null;
    this.reveal = null;
    this.rows = [];
    this.#repo = null;
    this.loading = false;
    this.complete = false;
    this.error = null;
  }
}

export const graph = new GraphStore();
