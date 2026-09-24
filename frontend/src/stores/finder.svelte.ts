import { findObject, type Found, type RepoId } from "$lib/ipc";

/** Find Object (Ctrl+P): one search at a time, the newest query's answer shown. */
class FinderStore {
  results = $state.raw<Found[]>([]);
  busy = $state(false);
  #token = 0;

  /** Rejects with the search's error, unless a newer query has replaced it. */
  async run(repo: RepoId | null, text: string): Promise<void> {
    const token = ++this.#token;
    if (repo === null || text.trim() === "") {
      this.results = [];
      this.busy = false;
      return;
    }
    this.busy = true;
    try {
      const found = await findObject(repo, text);
      if (token === this.#token) this.results = found;
    } catch (err) {
      if (token === this.#token) throw err;
    } finally {
      if (token === this.#token) this.busy = false;
    }
  }
}

export const finder = new FinderStore();
