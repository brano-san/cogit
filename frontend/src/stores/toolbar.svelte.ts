import { isMergedIntoHead, type RepoId } from "$lib/ipc";

/** What the button toolbar keeps between renders beyond the shared stores. */
class ToolbarStore {
  /** Whether HEAD already contains the selected commit; `undefined` until answered. */
  merged = $state<boolean | undefined>(undefined);
  #asked = 0;

  /** Clicking down the graph asks faster than the backend answers; stale replies lose. */
  async checkMerged(repo: RepoId | undefined, commit: string | null, head: string | null) {
    const ticket = ++this.#asked;
    this.merged = undefined;
    if (!repo || commit === null || commit === head) return;
    // A failed question offers the merge: Git then says why, rather than a button that
    // stays grey with no reason.
    const answer = await isMergedIntoHead(repo, commit).catch(() => false);
    if (ticket === this.#asked) this.merged = answer;
  }
}

export const toolbar = new ToolbarStore();
