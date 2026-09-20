import { headLabel, splitBranches } from "$lib/format";
import { CogitError, openRepository, repoStatus, type RepoSummary } from "$lib/ipc";

class RepositoryStore {
  current = $state<RepoSummary | null>(null);
  error = $state<CogitError | null>(null);
  busy = $state(false);

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

  close(): void {
    this.current = null;
    this.error = null;
  }
}

export const repository = new RepositoryStore();
