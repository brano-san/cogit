import {
  clearCommandLog,
  commandLog,
  commandOutcome,
  commandProblems,
  type CommandNotice,
  type GitOutput,
} from "$lib/ipc";
import { notices } from "$stores/notices.svelte";

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

  /** The record the output window is showing, or null when there is no window. It opens
      on request — Show Output in the notification window, a row of the history — and a
      failure is announced in the notification window, not here (doc/12-risks.md, R-178). */
  shown = $state.raw<GitOutput | null>(null);
  /** The one warning worth a toast; a second replaces it rather than stacking. */
  warning = $state.raw<CommandNotice | null>(null);

  /** Reads run side by side and answer in any order: only the newest may write, and Clear
      retires every read begun before it finished. */
  #logRead = 0;
  #countRead = 0;

  async refreshProblems(): Promise<void> {
    const asked = ++this.#countRead;
    const problems = await commandProblems();
    if (asked === this.#countRead) this.problems = problems;
  }

  get shownEntries(): GitOutput[] {
    return this.errorsOnly
      ? this.entries.filter((e) => isFailure(e) || isWarning(e))
      : this.entries;
  }

  async refresh(): Promise<void> {
    const asked = ++this.#logRead;
    const entries = await commandLog();
    if (asked === this.#logRead) this.entries = entries;
  }

  async clear(): Promise<void> {
    await clearCommandLog();
    this.#logRead += 1;
    this.#countRead += 1;
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

    if (event.severity === "failure") await notices.command(event);
    else if (event.severity === "warning") this.warning = event;
  }

  /** A record that has rotated out of the journal leaves the window closed. */
  async openRecord(id: number): Promise<void> {
    this.shown = await commandOutcome(id).catch(() => null);
  }

  async showWarning(): Promise<void> {
    if (this.warning === null) return;
    const wanted = this.warning;
    this.warning = null;
    await this.openRecord(wanted.id);
  }

  dismissWarning(): void {
    this.warning = null;
  }

  show(entry: GitOutput): void {
    this.shown = entry;
  }

  close(): void {
    this.shown = null;
  }
}

export const output = new OutputStore();
