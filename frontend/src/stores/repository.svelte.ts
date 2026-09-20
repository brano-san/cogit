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

/** Roots are remembered so the tree comes back with the same repositories after a restart. */
const REMEMBERED = "cogit:repositories";

function remembered(): string[] {
  try {
    const raw = localStorage.getItem(REMEMBERED);
    return raw ? (JSON.parse(raw) as string[]) : [];
  } catch {
    return [];
  }
}

function remember(roots: string[]): void {
  try {
    localStorage.setItem(REMEMBERED, JSON.stringify(roots));
  } catch {
    // A blocked store costs the user the list on restart, nothing more.
  }
}

class RepositoryStore {
  current = $state<RepoSummary | null>(null);
  error = $state<CogitError | null>(null);
  busy = $state(false);
  openRepos = $state.raw<RepoOverview[]>([]);

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
    this.busy = true;
    this.error = null;
    try {
      this.current = await openRepository(path);
    } catch (err) {
      this.error =
        err instanceof CogitError
          ? err
          : new CogitError({ kind: "internal", data: String(err) });
      this.current = null;
    } finally {
      this.busy = false;
    }
  }

  /** Staging changes only the counters; re-reading every ref for that is waste (R-24). */
  async refreshStatus(): Promise<void> {
    const repo = this.current?.repo;
    if (!repo) return;
    try {
      const status = await repoStatus(repo);
      if (this.current) this.current = { ...this.current, status };
    } catch {
      // Nothing actionable; the next full refresh reports it with its own error.
    }
  }

  async refresh(): Promise<void> {
    const root = this.current?.root;
    if (root) await this.open(root);
  }

  async refreshList(): Promise<void> {
    this.openRepos = await listRepositories();
    remember(this.openRepos.map((entry) => entry.root));
  }

  /** Reopens everything the previous session had, ignoring paths that are gone. */
  async restore(): Promise<string[]> {
    const failed: string[] = [];
    for (const root of remembered()) {
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
    if (this.current?.repo === repo) this.current = null;
    await this.refreshList();
  }

  close(): void {
    this.current = null;
    this.error = null;
  }
}

export const repository = new RepositoryStore();
