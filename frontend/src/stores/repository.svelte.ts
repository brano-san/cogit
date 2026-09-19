/**
 * The repository currently shown in the window.
 *
 * M3 replaces this with a list of repositories and grouping; for the vertical slice one
 * open repository is enough to prove the chain end to end.
 */

import { headLabel, splitBranches } from "$lib/format";
import { CogitError, openRepository, type RepoSummary } from "$lib/ipc";

class RepositoryStore {
  current = $state<RepoSummary | null>(null);
  /** Kept structured so the Git Error Dialog can show raw output (INV-05). */
  error = $state<CogitError | null>(null);
  busy = $state(false);

  /** Branches split for the References panel, which shows them under separate headings. */
  get localBranches() {
    return splitBranches(this.current?.branches ?? []).local;
  }

  get remoteBranches() {
    return splitBranches(this.current?.branches ?? []).remote;
  }

  /** Short description of HEAD for the status bar. */
  get headLabel(): string {
    return headLabel(this.current?.head);
  }

  async open(path: string): Promise<void> {
    this.busy = true;
    this.error = null;
    try {
      this.current = await openRepository(path);
    } catch (err) {
      // Anything that is not a CogitError is a bug in the bridge, not a Git failure —
      // wrap it rather than swallowing it, so it still reaches the user.
      this.error =
        err instanceof CogitError
          ? err
          : new CogitError({ kind: "internal", data: String(err) });
      this.current = null;
    } finally {
      this.busy = false;
    }
  }

  close(): void {
    this.current = null;
    this.error = null;
  }
}

export const repository = new RepositoryStore();
