import { commitSignatures, type RepoId, type SignatureStatus } from "$lib/ipc";

class SignatureStore {
  statuses = $state.raw<Map<string, SignatureStatus>>(new Map());

  #repo: RepoId | null = null;
  #running = false;
  #next: { repo: RepoId; window: readonly string[] } | null = null;

  async load(repo: RepoId, window: readonly string[]): Promise<void> {
    if (repo !== this.#repo) {
      this.#repo = repo;
      this.statuses = new Map();
    }
    this.#next = { repo, window };
    if (this.#running) return;
    this.#running = true;
    try {
      while (this.#next) {
        const { repo: asked, window: rows } = this.#next;
        this.#next = null;
        const missing = rows.filter((oid) => !this.statuses.has(oid));
        if (missing.length === 0) continue;
        const found = await commitSignatures(asked, [...missing]).catch(() => []);
        if (asked !== this.#repo) continue;
        const next = new Map(this.statuses);
        for (const status of found) next.set(status.oid, status);
        this.statuses = next;
      }
    } finally {
      this.#running = false;
    }
  }

  clear(): void {
    this.#repo = null;
    this.#next = null;
    this.statuses = new Map();
  }
}

export const signatures = new SignatureStore();
