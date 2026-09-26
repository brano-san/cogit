import { findObject, type Found, type RepoId } from "$lib/ipc";

/** Each letter would start another walk of the history; the search waits for a pause. */
export const SETTLE_MS = 150;

/** Find Object (Ctrl+P): the newest query is searched once typing pauses, and only its
    answer is shown. A walk already running is not stopped, only its answer dropped. */
class FinderStore {
  results = $state.raw<Found[]>([]);
  busy = $state(false);
  #token = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** Ends the wait of a query replaced before its pause was over. */
  #replaced: (() => void) | null = null;

  /** Rejects with the search's error, unless a newer query has replaced it. */
  async run(repo: RepoId | null, text: string): Promise<void> {
    const token = ++this.#token;
    clearTimeout(this.#timer);
    this.#replaced?.();
    this.#replaced = null;
    if (repo === null || text.trim() === "") {
      this.results = [];
      this.busy = false;
      return;
    }
    this.busy = true;
    const settled = await new Promise<boolean>((resolve) => {
      this.#replaced = () => resolve(false);
      this.#timer = setTimeout(() => {
        this.#replaced = null;
        resolve(true);
      }, SETTLE_MS);
    });
    if (!settled) return;
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
