import { groupFindings, visibleWarnings, type HealthWarning } from "$lib/health";
import { repositoryHealth, type RepoId } from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import type { IgnoredWarnings } from "$lib/suppressions";

const KEY = "health-ignored";

/** What is wrong with the open repository, stepped through like the failure queue. */
class HealthStore {
  ignored = $state.raw<IgnoredWarnings>({});
  at = $state(0);

  #all = $state.raw<HealthWarning[]>([]);
  #later = $state.raw<ReadonlySet<string>>(new Set());
  #root = $state<string | null>(null);

  get warnings(): HealthWarning[] {
    const mine = this.#root === null ? {} : (this.ignored[this.#root] ?? {});
    return visibleWarnings(this.#all, new Set(Object.keys(mine)), this.#later);
  }

  get current(): HealthWarning | undefined {
    return this.warnings[Math.min(this.at, this.warnings.length - 1)];
  }

  async loadIgnored(): Promise<void> {
    this.ignored = (await readKey<IgnoredWarnings>(KEY)) ?? {};
  }

  /** Every open starts afresh: "Remind me later" lasts until the repository opens again. */
  async check(repo: RepoId, root: string, name: string): Promise<void> {
    this.#root = root;
    this.#later = new Set();
    this.#all = [];
    this.at = 0;
    try {
      const findings = await repositoryHealth(repo);
      if (this.#root === root) this.#all = groupFindings(findings, name);
    } catch {
      // A check that cannot run has nothing to warn about; the open itself succeeded.
    }
  }

  remindLater(): void {
    const warning = this.current;
    if (!warning) return;
    this.#later = new Set([...this.#later, warning.id]);
    this.#clamp();
  }

  async ignore(): Promise<void> {
    const warning = this.current;
    const root = this.#root;
    if (!warning || root === null) return;
    this.ignored = {
      ...this.ignored,
      [root]: { ...(this.ignored[root] ?? {}), [warning.id]: warning.title },
    };
    this.#clamp();
    await writeKey(KEY, this.ignored);
  }

  async unignore(root: string, id: string): Promise<void> {
    const inner = { ...(this.ignored[root] ?? {}) };
    delete inner[id];
    const next = { ...this.ignored };
    if (Object.keys(inner).length === 0) delete next[root];
    else next[root] = inner;
    this.ignored = next;
    await writeKey(KEY, next);
  }

  step(delta: -1 | 1): void {
    this.at = Math.max(0, Math.min(this.at + delta, this.warnings.length - 1));
  }

  clear(): void {
    this.#root = null;
    this.#all = [];
    this.at = 0;
  }

  #clamp(): void {
    this.at = Math.max(0, Math.min(this.at, this.warnings.length - 1));
  }
}

export const health = new HealthStore();
