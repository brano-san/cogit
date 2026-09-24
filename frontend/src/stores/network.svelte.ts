import { authHost } from "$lib/pull-request";
import {
  fetchRemote,
  forgetToken,
  hasToken,
  listRemotes,
  pullRemote,
  pushRemote,
  remoteUrl,
  storeToken,
  type RepoId,
} from "$lib/ipc";

type Running = { id: number; repo: RepoId; label: string; progress: string | null };

class NetworkStore {
  remotes = $state.raw<string[]>([]);
  url = $state<string | null>(null);
  tokenStored = $state(false);
  /** Every operation on the network now, newest last. Each keeps its own line: the first
      to finish must not blank the status of one still running. */
  #running = $state.raw<Running[]>([]);
  #started = 0;

  /** The newest operation's label, shown in the status bar while it runs. */
  get running(): string | null {
    return this.#running.at(-1)?.label ?? null;
  }

  /** The last line Git printed for that operation. */
  get progress(): string | null {
    return this.#running.at(-1)?.progress ?? null;
  }

  /** Whose progress `progress` is; the Exit dialog must not give it to another repository. */
  get repo(): RepoId | null {
    return this.#running.at(-1)?.repo ?? null;
  }

  get tokenHost(): string | null {
    return authHost(this.url);
  }

  get primary(): string | null {
    return this.remotes.includes("origin") ? "origin" : (this.remotes[0] ?? null);
  }

  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;

  async refresh(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    const remotes = await listRemotes(repo);
    if (generation !== this.#generation) return;
    this.remotes = remotes;
    const url = this.primary ? await remoteUrl(repo, this.primary) : null;
    if (generation !== this.#generation) return;
    this.url = url;
    const host = this.tokenHost;
    const stored = host !== null && (await hasToken(host));
    if (generation === this.#generation) this.tokenStored = stored;
  }

  async storeToken(token: string): Promise<void> {
    const host = this.tokenHost;
    if (host === null) return;
    await storeToken(host, token);
    this.tokenStored = true;
  }

  async forgetToken(): Promise<void> {
    const host = this.tokenHost;
    if (host === null) return;
    await forgetToken(host);
    this.tokenStored = false;
  }

  async fetch(repo: RepoId, remote: string): Promise<void> {
    await this.run(repo, "Fetching", (onLine) => fetchRemote(repo, remote, onLine));
  }

  async pull(repo: RepoId, remote: string, ffOnly: boolean): Promise<void> {
    await this.run(repo, "Pulling", (onLine) => pullRemote(repo, remote, ffOnly, onLine));
  }

  async push(repo: RepoId, remote: string, force: boolean): Promise<void> {
    await this.run(repo, "Pushing", (onLine) => pushRemote(repo, remote, force, onLine));
  }

  async run(
    repo: RepoId,
    label: string,
    operation: (onLine: (line: string) => void) => Promise<unknown>,
  ): Promise<void> {
    const id = ++this.#started;
    this.#running = [...this.#running, { id, repo, label, progress: null }];
    // After `clear()` the operation is no longer listed, and its lines go nowhere.
    const onLine = (line: string) => {
      if (!this.#running.some((entry) => entry.id === id)) return;
      this.#running = this.#running.map((entry) => (entry.id === id ? { ...entry, progress: line } : entry));
    };
    try {
      await operation(onLine);
    } finally {
      this.#running = this.#running.filter((entry) => entry.id !== id);
    }
  }

  clear(): void {
    this.#generation += 1;
    this.remotes = [];
    this.url = null;
    this.tokenStored = false;
    this.#running = [];
  }
}

export const network = new NetworkStore();
