import { safetyLog, undoEntry, type RepoId, type SafetyEntry } from "$lib/ipc";

class SafetyStore {
  entries = $state.raw<SafetyEntry[]>([]);

  #undoing = false;

  /** What Undo would undo in `repo`: the journal holds every repository's entries, and
      Undo acts on the one on screen. */
  lastFor(repo: RepoId | null): SafetyEntry | null {
    if (repo === null) return null;
    return this.entries.find((entry) => entry.undoable && entry.repo === repo) ?? null;
  }

  async refresh(): Promise<void> {
    this.entries = await safetyLog();
  }

  /** Undo in the toolbar and the palette: the entry its tooltip names, by id. The newest
      one when the queue gets to it may be a write queued since, which is not what the user
      saw. A click while one runs is dropped. Null when nothing was undone. */
  async undoShown(repo: RepoId): Promise<SafetyEntry | null> {
    const shown = this.lastFor(repo);
    if (!shown || this.#undoing) return null;
    this.#undoing = true;
    try {
      return await this.undoOne(repo, shown.id);
    } finally {
      this.#undoing = false;
    }
  }

  /** Undoing an older entry is allowed: each recovery restores its own thing (T5.7). */
  async undoOne(repo: RepoId, id: number): Promise<SafetyEntry> {
    const entry = await undoEntry(repo, id);
    await this.refresh();
    return entry;
  }

  get forRepo(): (repo: RepoId) => SafetyEntry[] {
    return (repo) => this.entries.filter((entry) => entry.repo === repo);
  }
}

export const safety = new SafetyStore();
