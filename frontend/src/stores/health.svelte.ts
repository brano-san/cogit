import { groupFindings, visibleWarnings, type HealthWarning } from "$lib/health";
import { repositoryHealth, type RepoId } from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import type { IgnoredWarnings } from "$lib/suppressions";

const KEY = "health-ignored";

class HealthStore {
  ignored = $state.raw<IgnoredWarnings>({});

  #all = $state.raw<HealthWarning[]>([]);
  /** Per root, with the id it was open under: a switch keeps the id (R-351), and only a
      close and a new open change it. */
  #later = $state.raw<ReadonlyMap<string, { repo: RepoId; ids: ReadonlySet<string> }>>(new Map());
  #root = $state<string | null>(null);
  #checked: { repo: RepoId; name: string } | null = null;

  get warnings(): HealthWarning[] {
    const mine = this.#root === null ? {} : (this.ignored[this.#root] ?? {});
    const later = this.#root === null ? undefined : this.#later.get(this.#root)?.ids;
    return visibleWarnings(this.#all, new Set(Object.keys(mine)), later ?? new Set());
  }

  async loadIgnored(): Promise<void> {
    this.ignored = (await readKey<IgnoredWarnings>(KEY)) ?? {};
  }

  /** "Remind me later" lasts until the repository opens again, not until it is left. */
  async check(repo: RepoId, root: string, name: string): Promise<void> {
    this.#root = root;
    if (this.#later.get(root)?.repo !== repo) {
      this.#later = new Map([...this.#later, [root, { repo, ids: new Set<string>() }]]);
    }
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
    const root = this.#root;
    const held = root === null ? undefined : this.#later.get(root);
    if (root === null || !held) return;
    this.#later = new Map([...this.#later, [root, { ...held, ids: new Set([...held.ids, id]) }]]);
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
