import {
  CogitError,
  EMPTY_QUERY,
  loadCommits,
  type CommitQuery,
  type CommitRow,
  type GraphEdge,
  type LaneAssignment,
  type RepoId,
} from "$lib/ipc";

export interface GraphRow {
  commit: CommitRow;
  lane: LaneAssignment;
}

class GraphStore {
  /** `$state.raw`: 50 000 commits would otherwise become 50 000 reactive proxies. */
  rows = $state.raw<GraphRow[]>([]);
  edges = $state.raw<GraphEdge[]>([]);
  maxLane = $state(0);
  loading = $state(false);
  complete = $state(false);
  error = $state<CogitError | null>(null);

  /** Discriminates concurrent loads; chunks from a superseded stream are dropped. */
  #generation = 0;

  /** The search half only; the References panel contributes its half at load time. */
  query = $state.raw<CommitQuery>(EMPTY_QUERY);

  /** `null` is every ref. Owned by the References panel, folded into every load. */
  visibleRefs = $state.raw<string[] | null>(null);

  /** Asking twice for the same commit has to scroll twice, hence the counter. */
  reveal = $state.raw<{ oid: string; request: number } | null>(null);

  requestReveal(oid: string): void {
    this.reveal = { oid, request: (this.reveal?.request ?? 0) + 1 };
  }

  async load(repo: RepoId, query: CommitQuery = EMPTY_QUERY): Promise<void> {
    const generation = ++this.#generation;
    this.query = query;
    this.rows = [];
    this.edges = [];
    this.maxLane = 0;
    this.error = null;
    this.complete = false;
    this.loading = true;

    try {
      await loadCommits(repo, (chunk) => {
        if (generation !== this.#generation) return;

        if (chunk.commits.length > 0) {
          const incoming = chunk.commits.map((commit, index) => ({
            commit,
            lane: chunk.lanes[index]!,
          }));
          this.rows = [...this.rows, ...incoming];
          this.edges = [...this.edges, ...chunk.edges];
        }
        this.maxLane = Math.max(this.maxLane, chunk.maxLane);
        if (chunk.isLast) this.complete = true;
      }, { ...query, visibleRefs: this.visibleRefs });
    } catch (err) {
      if (generation === this.#generation) {
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
    this.edges = [];
    this.maxLane = 0;
    this.loading = false;
    this.complete = false;
    this.error = null;
  }
}

export const graph = new GraphStore();
