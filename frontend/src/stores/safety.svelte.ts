import { safetyLog, undoLast, type RepoId, type SafetyEntry } from "$lib/ipc";

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
}

export const safety = new SafetyStore();
