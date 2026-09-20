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

  query = $state.raw<CommitQuery>(EMPTY_QUERY);

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
      }, query);
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
    this.rows = [];
    this.edges = [];
    this.maxLane = 0;
    this.loading = false;
    this.complete = false;
    this.error = null;
  }
}

export const graph = new GraphStore();
