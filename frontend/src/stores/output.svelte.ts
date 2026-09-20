import { clearCommandLog, commandLog, commandProblems, type GitOutput } from "$lib/ipc";

export function isWarning(entry: GitOutput): boolean {
  return entry.exitCode === 0 && entry.stderr.trim() !== "";
}

export function isFailure(entry: GitOutput): boolean {
  return entry.exitCode !== 0;
}

class OutputStore {
  open = $state(false);
  errorsOnly = $state(false);
  entries = $state.raw<GitOutput[]>([]);

  problems = $state(0);

  async refreshProblems(): Promise<void> {
    this.problems = await commandProblems();
  }

  get shown(): GitOutput[] {
    return this.errorsOnly
      ? this.entries.filter((e) => isFailure(e) || isWarning(e))
      : this.entries;
  }

  async refresh(): Promise<void> {
    this.entries = await commandLog();
  }

  async clear(): Promise<void> {
    await clearCommandLog();
    this.entries = [];
    this.problems = 0;
  }

  toggle(): void {
    this.open = !this.open;
    if (this.open) void this.refresh();
  }
}

export const output = new OutputStore();
