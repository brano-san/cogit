import { headLabel, splitBranches } from "$lib/format";
import { CogitError, openRepository, type RepoSummary } from "$lib/ipc";

class RepositoryStore {
  current = $state<RepoSummary | null>(null);
  /** Kept structured so the Git Error Dialog can show raw output (INV-05). */
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

  /** Re-reads HEAD, branches and status after a mutation, keeping the same path. */
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
