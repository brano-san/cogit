import { groupFindings, visibleWarnings, type HealthWarning } from "$lib/health";
import { repositoryHealth, type RepoId } from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import type { IgnoredWarnings } from "$lib/suppressions";

const KEY = "health-ignored";

class HealthStore {
  ignored = $state.raw<IgnoredWarnings>({});

  #all = $state.raw<HealthWarning[]>([]);
  #later = $state.raw<ReadonlySet<string>>(new Set());
  #root = $state<string | null>(null);
  #checked: { repo: RepoId; name: string } | null = null;

  get warnings(): HealthWarning[] {
    const mine = this.#root === null ? {} : (this.ignored[this.#root] ?? {});
    return visibleWarnings(this.#all, new Set(Object.keys(mine)), this.#later);
  }

  async loadIgnored(): Promise<void> {
    this.ignored = (await readKey<IgnoredWarnings>(KEY)) ?? {};
  }

  /** Every open starts afresh: "Remind me later" lasts until the repository opens again. */
  async check(repo: RepoId, root: string, name: string): Promise<void> {
    this.#root = root;
    this.#later = new Set();
    this.#all = [];
    this.#checked = { repo, name };
    await this.recheck();
  }

  /** After a fix: the same repository asked again, with what was put off still put off. */
  async recheck(): Promise<void> {
    const checked = this.#checked;
    const root = this.#root;
    if (!checked || root === null) return;
    try {
      const findings = await repositoryHealth(checked.repo);
      if (this.#root === root) this.#all = groupFindings(findings, checked.name);
    } catch {
      // A check that cannot run has nothing to warn about; the open itself succeeded.
    }
  }

  remindLater(id: string): void {
    this.#later = new Set([...this.#later, id]);
  }

  async ignore(id: string): Promise<void> {
    const warning = this.#all.find((held) => held.id === id);
    const root = this.#root;
    if (!warning || root === null) return;
    this.ignored = {
      ...this.ignored,
      [root]: { ...(this.ignored[root] ?? {}), [warning.id]: warning.title },
    };
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

  clear(): void {
    this.#root = null;
    this.#checked = null;
    this.#all = [];
  }
}

export const health = new HealthStore();
