import {
  clearCommandLog,
  commandLog,
  commandOutcome,
  commandProblems,
  type CommandNotice,
  type GitOutput,
} from "$lib/ipc";

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

  /** The record the output window is showing, or null when there is no window. */
  shown = $state.raw<GitOutput | null>(null);
  /** Failures that arrived while the window was already up. */
  unread = $state(0);
  /** The one warning worth a toast; a second replaces it rather than stacking. */
  warning = $state.raw<CommandNotice | null>(null);

  #newest: number | null = null;
  #loading: number | null = null;

  async refreshProblems(): Promise<void> {
    this.problems = await commandProblems();
  }

  get shownEntries(): GitOutput[] {
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

  /** Every git command ends here. What happens next is only ever decided by severity. */
  async notice(event: CommandNotice): Promise<void> {
    if (event.severity !== "success") this.problems += 1;
    if (this.open) void this.refresh();

    if (event.severity === "failure") await this.raise(event.id);
    else if (event.severity === "warning") this.warning = event;
  }

  /** One window, however many failures: a stack of them buries the first one, which is
   *  usually the one that explains the rest. A failure reaches this twice — once as the
   *  event, once as the rejected call — so the same record never counts twice. */
  async raise(id: number): Promise<void> {
    if (this.shown?.id === id || this.#newest === id || this.#loading === id) return;
    if (this.shown !== null) {
      this.#newest = id;
      this.unread += 1;
      return;
    }
    this.#loading = id;
    const full = await commandOutcome(id);
    this.#loading = null;
    if (full) this.shown = full;
  }

  async showNewest(): Promise<void> {
    if (this.#newest === null) return;
    const wanted = this.#newest;
    this.#newest = null;
    this.unread = 0;
    this.shown = await commandOutcome(wanted);
  }

  async showWarning(): Promise<void> {
    if (this.warning === null) return;
    const wanted = this.warning;
    this.warning = null;
    this.shown = await commandOutcome(wanted.id);
  }

  /** From the history list, where the reader went looking for something already closed. */
  show(entry: GitOutput): void {
    this.shown = entry;
  }

  close(): void {
    this.shown = null;
    this.unread = 0;
    this.#newest = null;
  }

  dismissWarning(): void {
    this.warning = null;
  }
}

export const output = new OutputStore();
