import {
  type CogitError,
  stashContents,
  type DiffSpec,
  type StashContents,
  type RepoId,
  toCogitError,
} from "$lib/ipc";

/** The stash the Files panel is showing. Separate from `commit`: a stash is not a commit
    the user selected, and the two must not fight over the panel. */
class StashViewStore {
  contents = $state.raw<StashContents | null>(null);
  loading = $state(false);
  error = $state<CogitError | null>(null);

  /** Clicking down the stash list outruns the backend; stale answers lose. */
  #generation = 0;

  async select(repo: RepoId, index: number): Promise<void> {
    const generation = ++this.#generation;
    this.error = null;
    this.loading = true;

    try {
      const found = await stashContents(repo, index);
      if (generation !== this.#generation) return;
      this.contents = found;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.contents = null;
      this.error = toCogitError(err);
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
  }

  /** Which two commits a file of the given part sits between. */
  spec(part: "worktree" | "index" | "untracked"): DiffSpec | null {
    const held = this.contents;
    if (!held) return null;
    if (part === "worktree") return { kind: "commitVsCommit", a: held.base, b: held.worktreeRev };
    if (part === "index") return { kind: "commitVsCommit", a: held.base, b: held.indexRev };
    return held.untrackedRev === null
      ? null
      : { kind: "commitVsCommit", a: held.base, b: held.untrackedRev };
  }

  clear(): void {
    this.#generation += 1;
    this.contents = null;
    this.loading = false;
    this.error = null;
  }
}

export const stashView = new StashViewStore();
