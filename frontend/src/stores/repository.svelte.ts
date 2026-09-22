import { headLabel, splitBranches } from "$lib/format";
import {
  closeRepository,
  CogitError,
  listRepositories,
  openRepository,
  repoStatus,
  type RepoId,
  type RepoOverview,
  type RepoSummary,
} from "$lib/ipc";
import { trace } from "$lib/trace";
import { session } from "$stores/session.svelte";

/** Opening a repository is one transition, and every panel reads the result of it from
    here rather than catching an event as it goes past. A panel mounted late sees the
    same thing as one mounted early, which is what the Graph panel did not (R-98).

    `repo` rides along on `opening` and `failed` so that re-reading an open repository —
    which the file watcher does on every ref move — cannot blank the panels while it is
    in flight or if it goes wrong. */
export type RepoPhase =
  | { kind: "closed" }
  | { kind: "opening"; root: string; repo: RepoSummary | null }
  | { kind: "open"; repo: RepoSummary }
  | { kind: "failed"; root: string; error: CogitError; repo: RepoSummary | null };

/** Long enough that a cold repository with submodules is not called a failure, short
    enough that nobody watches a spinner wondering whether it is still alive. */
const OPEN_TIMEOUT_MS = 30_000;

function asCogitError(err: unknown): CogitError {
  return err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
}

class RepositoryStore {
  phase = $state.raw<RepoPhase>({ kind: "closed" });
  openRepos = $state.raw<RepoOverview[]>([]);

  readonly openTimeoutMs = OPEN_TIMEOUT_MS;

  /** Only the newest open may write the phase; an older one that finishes late is
      dropped, whichever way it ends. */
  #ticket = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;

  get current(): RepoSummary | null {
    return this.phase.kind === "closed" ? null : this.phase.repo;
  }

  get busy(): boolean {
    return this.phase.kind === "opening";
  }

  get error(): CogitError | null {
    return this.phase.kind === "failed" ? this.phase.error : null;
  }

  get localBranches() {
    return splitBranches(this.current?.branches ?? []).local;
  }

  get remoteBranches() {
    return splitBranches(this.current?.branches ?? []).remote;
  }

  get headLabel(): string {
    return headLabel(this.current?.head);
  }

  async open(path: string): Promise<void> {
    const ticket = this.#begin(path);
    try {
      const repo = await openRepository(path);
      trace(`open:${path}`, `backend answered, ticket ${ticket}, ${repo.branches.length} refs`);
      this.#settle(ticket, { kind: "open", repo });
    } catch (err) {
      trace(`open:${path}`, `backend refused, ticket ${ticket}: ${String(err)}`);
      this.#settle(ticket, {
        kind: "failed",
        root: path,
        error: asCogitError(err),
        repo: this.current,
      });
    }
  }

  #begin(root: string): number {
    const ticket = ++this.#ticket;
    this.#disarm();
    this.phase = { kind: "opening", root, repo: this.current };
    trace(`open:${root}`, `phase → opening, ticket ${ticket}`);
    // A backend that never answers is a bug of its own, but it must not read as
    // "still working" for ever. A real answer arriving later still wins.
    this.#timer = setTimeout(() => {
      if (this.#ticket !== ticket || this.phase.kind !== "opening") return;
      trace(`open:${root}`, `phase → failed, timed out after ${OPEN_TIMEOUT_MS}ms`);
      this.phase = {
        kind: "failed",
        root,
        error: new CogitError({
          kind: "internal",
          data: `Opening ${root} is taking longer than ${Math.round(OPEN_TIMEOUT_MS / 1000)}s. It may still be running; the log says how far it got.`,
        }),
        repo: this.current,
      };
    }, OPEN_TIMEOUT_MS);
    return ticket;
  }

  #settle(ticket: number, phase: RepoPhase): void {
    if (this.#ticket !== ticket) {
      trace("open", `ticket ${ticket} is stale, ${this.#ticket} is current; answer dropped`);
      return;
    }
    this.#disarm();
    this.phase = phase;
    trace("open", `phase → ${phase.kind}, ticket ${ticket}`);
  }

  #disarm(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }

  /** Staging changes only the counters; re-reading every ref for that is waste (R-24). */
  async refreshStatus(): Promise<void> {
    const repo = this.current?.repo;
    if (!repo) return;
    try {
      const status = await repoStatus(repo);
      const open = this.current;
      if (open && open.repo === repo) this.#replace({ ...open, status });
    } catch {
      // Nothing actionable; the next full refresh reports it with its own error.
    }
  }

  /** Takes a repository somebody else opened — a submodule reached from the tree —
      without asking the backend to open it again. */
  adopt(repo: RepoSummary): void {
    this.#ticket += 1;
    this.#disarm();
    this.phase = { kind: "open", repo };
  }

  /** The repository is the same one; only its contents were re-read. */
  #replace(repo: RepoSummary): void {
    if (this.phase.kind === "closed") return;
    this.phase = this.phase.kind === "open" ? { kind: "open", repo } : { ...this.phase, repo };
  }

  async refresh(): Promise<void> {
    const root = this.current?.root;
    if (root) await this.open(root);
  }

  async refreshList(): Promise<void> {
    this.openRepos = await listRepositories();
    session.remember(this.openRepos.map((entry) => entry.root));
  }

  /** Reopens everything the previous session had, ignoring paths that are gone. */
  async restore(): Promise<string[]> {
    const failed: string[] = [];
    for (const root of session.repositories) {
      try {
        await openRepository(root);
      } catch {
        failed.push(root);
      }
    }
    await this.refreshList();
    return failed;
  }

  async closeOne(repo: RepoId): Promise<void> {
    await closeRepository(repo);
    if (this.current?.repo === repo) this.close();
    await this.refreshList();
  }

  close(): void {
    this.#ticket += 1;
    this.#disarm();
    this.phase = { kind: "closed" };
  }
}

export const repository = new RepositoryStore();
