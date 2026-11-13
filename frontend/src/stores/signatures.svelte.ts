import { SvelteMap } from "svelte/reactivity";
import { commitSignatures, type RepoId, type SignatureStatus } from "$lib/ipc";

class SignatureStore {
  /** Filled in place: copying the whole map per batch grew with every commit scrolled past. */
  readonly statuses = new SvelteMap<string, SignatureStatus>();

  #repo: RepoId | null = null;
  #asked = new Set<string>();
  #running = false;
  #next: { repo: RepoId; window: readonly string[] } | null = null;

  async load(repo: RepoId, window: readonly string[]): Promise<void> {
    if (repo !== this.#repo) {
      this.#repo = repo;
      this.#asked = new Set();
      this.statuses.clear();
    }
    this.#next = { repo, window };
    if (this.#running) return;
    this.#running = true;
    try {
      while (this.#next) {
        const { repo: asked, window: rows } = this.#next;
        this.#next = null;
        const missing = rows.filter((oid) => !this.#asked.has(oid));
        if (missing.length === 0) continue;
        for (const oid of missing) this.#asked.add(oid);
        const found = await commitSignatures(asked, [...missing]).catch(() => []);
        if (asked !== this.#repo) continue;
        for (const status of found) this.statuses.set(status.oid, status);
      }
    } finally {
      this.#running = false;
    }
  }

  clear(): void {
    this.#repo = null;
    this.#asked = new Set();
    this.#next = null;
    this.statuses.clear();
  }
}

export const signatures = new SignatureStore();
