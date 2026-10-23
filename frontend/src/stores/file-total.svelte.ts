import { fileCount, type RepoId } from "$lib/ipc";

/** Commits remembered at most; past it the memory starts again. */
const KEPT = 500;

/** How many files the revision Files shows holds, for "N files hidden" (R-871). A commit's
    count never changes, so it is kept; the index's is asked again on every refresh. */
class FileTotalStore {
  value = $state<number | null>(null);

  #commits = new Map<string, number>();
  #asked = 0;

  /** `rev` null: the index. The last count stays shown until the new one comes. */
  async read(repo: RepoId, rev: string | null): Promise<void> {
    const asked = ++this.#asked;
    const key = rev === null ? null : `${repo}:${rev}`;
    const known = key === null ? undefined : this.#commits.get(key);
    if (known !== undefined) {
      this.value = known;
      return;
    }
    try {
      const count = await fileCount(repo, rev);
      if (key !== null) {
        if (this.#commits.size >= KEPT) this.#commits.clear();
        this.#commits.set(key, count);
      }
      if (asked === this.#asked) this.value = count;
    } catch {
      // The note goes rather than show a count of another revision.
      if (asked === this.#asked) this.value = null;
    }
  }

  clear(): void {
    this.#asked += 1;
    this.value = null;
  }
}

export const fileTotal = new FileTotalStore();
