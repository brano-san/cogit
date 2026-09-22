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
  /** Every failure that has not been read yet, oldest first. `Close` used to throw the
      rest away; now it moves along the queue (doc/12-risks.md, R-135). */
  queue = $state.raw<number[]>([]);
  /** Which of them is on screen. */
  at = $state(0);
  /** How many times the one on screen has happened. Fifteen identical pull failures are
      one thing that happened fifteen times. */
  repeats = $state(1);

  /** Failures that arrived while the window was already up. */
  unread = $state(0);
  /** The one warning worth a toast; a second replaces it rather than stacking. */
  warning = $state.raw<CommandNotice | null>(null);

  #loading: number | null = null;
  #lastSummary: string | null = null;

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

    if (event.severity === "failure") await this.raise(event.id, event.summary);
    else if (event.severity === "warning") this.warning = event;
  }

  /** One window, however many failures: a stack of them buries the first one, which is
   *  usually the one that explains the rest. A failure reaches this twice — once as the
   *  event, once as the rejected call — so the same record never counts twice. */
  async raise(id: number, summary?: string): Promise<void> {
    if (this.queue.includes(id) || this.#loading === id) return;

    // The same failure again is a count, not another entry.
    if (summary !== undefined && summary === this.#lastSummary && this.queue.length > 0) {
      this.repeats += 1;
      return;
    }
    if (summary !== undefined) {
      this.#lastSummary = summary;
      this.repeats = 1;
    }

    this.queue = [...this.queue, id];
    if (this.shown !== null) {
      this.unread += 1;
      return;
    }
    this.at = this.queue.length - 1;
    await this.#showAt(this.at);
  }

  async #showAt(index: number): Promise<void> {
    const id = this.queue[index];
    if (id === undefined) {
      this.shown = null;
      return;
    }
    this.#loading = id;
    const full = await commandOutcome(id);
    this.#loading = null;
    this.shown = full;
  }

  /** Previous or next failure in the queue, without losing the others. */
  async step(by: number): Promise<void> {
    if (this.queue.length === 0) return;
    const wanted = Math.min(Math.max(this.at + by, 0), this.queue.length - 1);
    if (wanted === this.at) return;
    this.at = wanted;
    this.unread = 0;
    await this.#showAt(wanted);
  }

  /** `Close` on one record: drop it and show the next, or close the window on the last. */
  async dismissShown(): Promise<void> {
    const rest = this.queue.filter((_, index) => index !== this.at);
    this.queue = rest;
    if (rest.length === 0) {
      this.close();
      return;
    }
    this.at = Math.min(this.at, rest.length - 1);
    this.unread = Math.max(this.unread - 1, 0);
    await this.#showAt(this.at);
  }

  async showNewest(): Promise<void> {
    if (this.queue.length === 0) return;
    this.at = this.queue.length - 1;
    this.unread = 0;
    await this.#showAt(this.at);
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
    this.queue = [];
    this.at = 0;
    this.repeats = 1;
    this.#lastSummary = null;
  }

  dismissWarning(): void {
    this.warning = null;
  }
}

export const output = new OutputStore();
