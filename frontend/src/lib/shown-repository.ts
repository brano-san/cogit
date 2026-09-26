import type { RepoId } from "$lib/ipc";

/** Tells the backend which repository the panels show, the only one it watches (R-351).
    One call at a time and only the latest wish: two calls in flight can land in either
    order, and the older one landing last would watch the repository just left. */
export class ShownRepository {
  readonly #send: (repo: RepoId | null) => Promise<void>;
  #wanted: RepoId | null = null;
  #sent: RepoId | null | undefined = undefined;
  #sending = false;

  constructor(send: (repo: RepoId | null) => Promise<void>) {
    this.#send = send;
  }

  set(repo: RepoId | null): void {
    this.#wanted = repo;
    void this.#flush();
  }

  async #flush(): Promise<void> {
    if (this.#sending) return;
    this.#sending = true;
    try {
      while (this.#sent !== this.#wanted) {
        const repo = this.#wanted;
        this.#sent = repo;
        // A failed call leaves the old watchers running, which costs a folder lock, not
        // a wrong panel; the backend logs why.
        await this.#send(repo).catch(() => undefined);
      }
    } finally {
      this.#sending = false;
    }
  }
}
