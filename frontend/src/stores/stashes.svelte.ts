import {
  listStashes,
  stashApply,
  stashDrop,
  stashKeepingWorktree,
  stashPush,
  type RepoId,
  type StashEntry,
} from "$lib/ipc";
import { stashRequest, type StashChoice } from "$lib/stash-modes";

class StashStore {
  entries = $state.raw<StashEntry[]>([]);
  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;
  /** Bumped by `clear()`: a write that finishes after it reads nothing back, since the
      panels it would read into belong to another repository by now. */
  #cleared = 0;

  async refresh(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    const entries = await listStashes(repo);
    if (generation === this.#generation) this.entries = entries;
  }

  async push(repo: RepoId, message: string, includeUntracked: boolean): Promise<void> {
    await this.#then(repo, () => stashPush(repo, { message, includeUntracked, keepIndex: false }));
  }

  /** One of the Stash dialog's three modes (#29). */
  async save(repo: RepoId, choice: StashChoice): Promise<void> {
    const request = stashRequest(choice);
    await this.#then(repo, async () => {
      if (request.kind === "keepWorktree") await stashKeepingWorktree(repo, request.message);
      else {
        const { message, includeUntracked, keepIndex } = request;
        await stashPush(repo, { message, includeUntracked, keepIndex });
      }
    });
  }

  async apply(repo: RepoId, index: number, pop: boolean): Promise<void> {
    await this.#then(repo, () => stashApply(repo, index, pop));
  }

  async drop(repo: RepoId, index: number): Promise<void> {
    await this.#then(repo, () => stashDrop(repo, index));
  }

  clear(): void {
    this.#generation += 1;
    this.#cleared += 1;
    this.entries = [];
  }

  async #then(repo: RepoId, write: () => Promise<unknown>): Promise<void> {
    const cleared = this.#cleared;
    await write();
    if (cleared === this.#cleared) await this.refresh(repo);
  }
}

export const stashes = new StashStore();
