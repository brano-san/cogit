import type { GitCandidate } from "$lib/ipc/bindings";
import type { Probe } from "$lib/git-check";
import type { Picked } from "$lib/git-missing";

export interface GitMissingDeps {
  probe: (path: string) => Promise<Probe>;
  candidates: () => Promise<GitCandidate[]>;
  /** Runs git from this path from the next command on. */
  apply: (path: string) => Promise<void>;
  /** Saves Preferences ▸ Git executable. */
  save: (path: string) => Promise<void>;
}

export type GitDialog =
  | { kind: "missing"; reason: string | null }
  | { kind: "old"; version: string; minimum: string };

/** The startup git check and its dialog: opens once per session, `Continue without git`
    silences it until the next start, and a git that works closes it. */
export class GitMissingStore {
  open = $state.raw<GitDialog | null>(null);
  /** The footer's `Git not found`: stays while the session runs without a working git. */
  missing = $state(false);
  candidates = $state.raw<GitCandidate[] | null>(null);
  picked = $state.raw<Picked | null>(null);

  #shown = false;
  #latest = 0;

  constructor(private readonly deps: GitMissingDeps) {}

  /** Right after the settings are read; never throws, never waits for the repositories. */
  async check(path: string): Promise<void> {
    let answer: Probe;
    try {
      answer = await this.deps.probe(path);
    } catch (err) {
      answer = { valid: false, version: null, error: err instanceof Error ? err.message : String(err), olderThan: null };
    }
    if (!answer.valid) {
      this.missing = true;
      if (!this.#shown) void this.#show({ kind: "missing", reason: answer.error });
      return;
    }
    this.missing = false;
    if (answer.olderThan && answer.version && !this.#shown) {
      this.#shown = true;
      this.open = { kind: "old", version: answer.version, minimum: answer.olderThan };
    }
  }

  /** `Fix…` on a failed command and the footer: ignores `Continue without git`. */
  reopen(reason: string | null): void {
    this.missing = true;
    this.candidates = null;
    void this.#show({ kind: "missing", reason });
  }

  continueWithout(): void {
    this.open = null;
  }

  /** The user picked or clicked a git: probe it now and use it if it works. */
  async choose(path: string): Promise<boolean> {
    const mine = ++this.#latest;
    this.picked = { state: "checking", path };
    let answer: Probe;
    try {
      answer = await this.deps.probe(path);
    } catch (err) {
      answer = { valid: false, version: null, error: err instanceof Error ? err.message : String(err), olderThan: null };
    }
    if (mine !== this.#latest) return false;
    if (!answer.valid || answer.version === null) {
      this.picked = { state: "bad", path, reason: answer.error ?? "Not a working git." };
      return false;
    }
    this.picked = { state: "ok", path, version: answer.version };
    await this.deps.apply(path);
    await this.deps.save(path);
    this.missing = false;
    this.open = null;
    return true;
  }

  async #show(dialog: GitDialog): Promise<void> {
    this.#shown = true;
    this.picked = null;
    this.open = dialog;
    if (this.candidates !== null) return;
    try {
      this.candidates = await this.deps.candidates();
    } catch {
      this.candidates = [];
    }
  }
}
