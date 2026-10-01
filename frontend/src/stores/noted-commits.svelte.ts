import type { RepoId } from "$lib/ipc";
import { commitNotes, notedCommits } from "$lib/ipc/ref-ops";
import { noteTip } from "$lib/notes";

/** Which commits carry a git note, read once per graph load; the text of one is asked for
    when the pointer reaches its icon. A repository without notes costs the one call. */
export class NotedCommits {
  oids = $state.raw<ReadonlySet<string>>(new Set());
  texts = $state.raw<ReadonlyMap<string, string>>(new Map());
  #repo: RepoId | null = null;
  #serial = 0;

  has(oid: string): boolean {
    return this.oids.size > 0 && this.oids.has(oid);
  }

  async refresh(repo: RepoId): Promise<void> {
    const serial = ++this.#serial;
    if (repo !== this.#repo) this.oids = new Set();
    this.#repo = repo;
    const list = await notedCommits(repo).catch(() => null);
    if (serial !== this.#serial || list === null) return;
    this.oids = new Set(list);
    this.texts = new Map();
  }

  clear(): void {
    this.#serial++;
    this.#repo = null;
    if (this.oids.size > 0) this.oids = new Set();
  }

  async load(oid: string): Promise<void> {
    const repo = this.#repo;
    if (repo === null || this.texts.has(oid)) return;
    const serial = this.#serial;
    const text = noteTip(await commitNotes(repo, oid).catch(() => []));
    if (serial !== this.#serial) return;
    this.texts = new Map(this.texts).set(oid, text);
  }
}

export const notedCommitsStore = new NotedCommits();
