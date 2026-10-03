import {
  commandOutcome,
  onErrorReported,
  onErrorsAction,
  openErrorsWindow,
  publishErrorQueue,
  reportErrorEntry,
  type ErrorAction,
  type ErrorEntry,
} from "$lib/ipc";
import { entryOf, pushEntry } from "$lib/error-window";
import { untrack } from "svelte";

/** A warning younger than this is not cleared by a conflict list that is still the one read
    before the command ran. */
const FRESH_MS = 3_000;

/** What a failed or conflicted command is known by, from the event or from the rejected
    call: the journal fills in the rest. */
interface Reported {
  id: number;
  repo: string;
  operation: string;
  summary: string;
  command?: string;
  stoppedOnConflicts?: boolean;
}

/** Failed git commands, and the ones that stopped on conflicts, in the Errors window: one
    window, one list (doc/12-risks.md, R-639). The main window owns the queue; every other
    page reports into it. What the taskbar shows reads `errorCount`, `unviewed` and
    `warningPresent`. */
class ErrorWindowStore {
  entries = $state.raw<ErrorEntry[]>([]);
  #viewed = $state.raw<ReadonlySet<number>>(new Set());
  #owner = false;
  #loading = new Set<number>();
  #born = new Map<number, number>();
  /** A burst of failures opens the window once: a call already on its way covers them. */
  #opening: Promise<void> | null = null;

  /** Failed commands still waiting: the footer's `Error` burns while there are any. */
  get errorCount(): number {
    return this.entries.filter((entry) => entry.kind === "error").length;
  }

  /** Entries nobody has had on screen yet, errors and warnings alike. */
  get unviewed(): number {
    return this.entries.filter((entry) => !this.#viewed.has(entry.id)).length;
  }

  get unviewedErrors(): number {
    return this.entries.filter((entry) => entry.kind === "error" && !this.#viewed.has(entry.id))
      .length;
  }

  /** A command stopped on conflicts and the user has neither opened them nor resolved them. */
  get warningPresent(): boolean {
    return this.entries.some((entry) => entry.kind === "warning");
  }

  /** The main window calls this once. The returned function stops listening. */
  async own(showConflicts: (repo: string) => void): Promise<() => void> {
    this.#owner = true;
    const stops = await Promise.all([
      onErrorReported((entry) => this.add(entry)),
      onErrorsAction((action, id) => this.#act(action, id, showConflicts)),
    ]);
    return () => {
      this.#owner = false;
      for (const stop of stops) stop();
    };
  }

  /** A failed command arrives twice — as the event and as the rejected call — and is queued
      once. Called from effects, so untracked. */
  async command(run: Reported): Promise<void> {
    const known = untrack(() => this.entries.some((entry) => entry.id === run.id));
    if (known || this.#loading.has(run.id)) return;
    this.#loading.add(run.id);
    const record = await commandOutcome(run.id).catch(() => null);
    this.#loading.delete(run.id);
    const entry = entryOf({
      command: "",
      stoppedOnConflicts: false,
      ...run,
      ...(record ?? {}),
    });
    if (this.#owner) this.add(entry);
    else await reportErrorEntry(entry).catch(() => {});
  }

  add(entry: ErrorEntry): void {
    untrack(() => {
      this.#born.set(entry.id, Date.now());
      this.entries = pushEntry(this.entries, entry);
      this.#publish();
    });
    this.#open();
  }

  #open(): void {
    if (this.#opening) return;
    this.#opening = openErrorsWindow()
      .then(() => {})
      .catch(() => {})
      .finally(() => (this.#opening = null));
  }

  /** Warnings of a repository whose conflicts are all resolved go away with them. */
  conflictsResolved(repo: string): void {
    const now = Date.now();
    // The journal names the root with the OS's slashes, the repository list with `/`.
    const same = (a: string, b: string) => a.replace(/\\/g, "/") === b.replace(/\\/g, "/");
    const stale = (entry: ErrorEntry) =>
      entry.kind === "warning" &&
      same(entry.repo, repo) &&
      now - (this.#born.get(entry.id) ?? 0) > FRESH_MS;
    if (!this.entries.some(stale)) return;
    this.entries = this.entries.filter((entry) => !stale(entry));
    this.#publish();
  }

  #act(action: ErrorAction, id: number | null, showConflicts: (repo: string) => void): void {
    if (action === "ready") return this.#publish();
    if (action === "closed") {
      // What the window had is seen and goes; an entry that came in while it was closing
      // was never on screen, and the window opens again for it.
      this.entries = this.entries.filter((entry) => !this.#viewed.has(entry.id));
      this.#viewed = new Set();
      this.#publish();
      // Not coalesced: an open on its way may be the one this close has just undone.
      if (this.entries.length > 0) void openErrorsWindow().catch(() => {});
      return;
    }
    if (id === null) return;
    if (action === "viewed") {
      this.#viewed = new Set(this.#viewed).add(id);
      return;
    }
    const entry = this.entries.find((held) => held.id === id);
    this.entries = this.entries.filter((held) => held.id !== id);
    this.#publish();
    if (action === "showConflicts" && entry) showConflicts(entry.repo);
  }

  #publish(): void {
    void publishErrorQueue(this.entries).catch(() => {});
  }
}

export const errorWindow = new ErrorWindowStore();
