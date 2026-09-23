import {
  blameFile,
  CogitError,
  fileRevisions,
  lineHistory,
  type BlameLine,
  type CommitRow,
  type LineVersion,
} from "$lib/ipc";
import { clampLine, type BlameRequest } from "$lib/blame-window";

/** How long the cursor rests on a line before its history is asked for: arrowing down a
    file must not start one `git log -L` per line passed. */
export const HISTORY_DELAY_MS = 150;

function message(err: unknown): string {
  return err instanceof CogitError ? err.message : String(err);
}

/** One per window: the Blame window is its own webview, and this is all it holds. */
class BlameWindowStore {
  request = $state.raw<BlameRequest | null>(null);
  /** The versions of the file, newest first: `View Commit` and `Changes Since` list them. */
  revisions = $state.raw<CommitRow[]>([]);
  revisionsError = $state<string | null>(null);
  /** The commit whose version of the file is on screen. */
  view = $state("");
  since = $state<string | null>(null);
  lines = $state.raw<BlameLine[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  /** Index into `lines`. */
  cursor = $state(0);
  history = $state.raw<LineVersion[]>([]);
  historyLoading = $state(false);
  historyError = $state<string | null>(null);
  historyShown = $state(true);

  #blameGeneration = 0;
  #historyGeneration = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;

  async open(request: BlameRequest): Promise<void> {
    this.request = request;
    await Promise.all([this.#loadRevisions(), this.show(request.rev, 1)]);
  }

  /** Another version of the file; the cursor keeps its line number where the file allows. */
  async show(oid: string, line = this.cursor + 1): Promise<void> {
    const request = this.request;
    if (!request) return;
    const generation = ++this.#blameGeneration;
    this.view = oid;
    this.loading = true;
    this.error = null;
    try {
      const found = await blameFile(request.repo, request.path, oid);
      if (generation !== this.#blameGeneration) return;
      this.lines = found;
      this.cursor = clampLine(line, found.length);
    } catch (err) {
      if (generation !== this.#blameGeneration) return;
      this.lines = [];
      this.history = [];
      this.error = message(err);
    } finally {
      if (generation === this.#blameGeneration) this.loading = false;
    }
    if (generation === this.#blameGeneration && this.error === null) await this.loadHistory();
  }

  async refresh(): Promise<void> {
    await Promise.all([this.#loadRevisions(), this.show(this.view)]);
  }

  moveTo(index: number): void {
    this.cursor = index;
    this.#cancelTimer();
    this.#timer = setTimeout(() => void this.loadHistory(), HISTORY_DELAY_MS);
  }

  async loadHistory(): Promise<void> {
    this.#cancelTimer();
    const request = this.request;
    if (!request || !this.historyShown || this.lines.length === 0) return;
    const generation = ++this.#historyGeneration;
    this.historyLoading = true;
    this.historyError = null;
    try {
      const found = await lineHistory(request.repo, request.path, this.view, this.cursor + 1);
      if (generation !== this.#historyGeneration) return;
      this.history = found;
    } catch (err) {
      if (generation !== this.#historyGeneration) return;
      this.history = [];
      this.historyError = message(err);
    } finally {
      if (generation === this.#historyGeneration) this.historyLoading = false;
    }
  }

  toggleHistory(): void {
    this.historyShown = !this.historyShown;
    if (this.historyShown) void this.loadHistory();
  }

  /** Tests only: the store is a singleton per webview. */
  reset(): void {
    this.#cancelTimer();
    this.#blameGeneration += 1;
    this.#historyGeneration += 1;
    this.request = null;
    this.revisions = [];
    this.revisionsError = null;
    this.view = "";
    this.since = null;
    this.lines = [];
    this.loading = false;
    this.error = null;
    this.cursor = 0;
    this.history = [];
    this.historyLoading = false;
    this.historyError = null;
    this.historyShown = true;
  }

  #cancelTimer(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }

  async #loadRevisions(): Promise<void> {
    const request = this.request;
    if (!request) return;
    try {
      this.revisions = await fileRevisions(request.repo, request.path, request.rev);
      this.revisionsError = null;
    } catch (err) {
      this.revisions = [];
      this.revisionsError = message(err);
    }
  }
}

export const blameWindow = new BlameWindowStore();
