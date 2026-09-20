import { safetyLog, undoEntry, undoLast, type RepoId, type SafetyEntry } from "$lib/ipc";

class SafetyStore {
  entries = $state.raw<SafetyEntry[]>([]);

  get last(): SafetyEntry | null {
    return this.entries.find((entry) => entry.undoable) ?? null;
  }

  async refresh(): Promise<void> {
    this.entries = await safetyLog();
  }

  async undo(repo: RepoId): Promise<SafetyEntry> {
    const entry = await undoLast(repo);
    await this.refresh();
    return entry;
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
