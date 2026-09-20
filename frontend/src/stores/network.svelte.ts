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

class NetworkStore {
  remotes = $state.raw<string[]>([]);
  url = $state<string | null>(null);
  /** The last line Git printed, shown in the status bar while the operation runs. */
  progress = $state<string | null>(null);
  running = $state<string | null>(null);
  tokenStored = $state(false);

  get tokenHost(): string | null {
    return authHost(this.url);
  }

  get primary(): string | null {
    return this.remotes.includes("origin") ? "origin" : (this.remotes[0] ?? null);
  }

  async refresh(repo: RepoId): Promise<void> {
    this.remotes = await listRemotes(repo);
    this.url = this.primary ? await remoteUrl(repo, this.primary) : null;
    const host = this.tokenHost;
    this.tokenStored = host !== null && (await hasToken(host));
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
    await this.run("Fetching", () => fetchRemote(repo, remote, (l) => (this.progress = l)));
  }

  async pull(repo: RepoId, remote: string, ffOnly: boolean): Promise<void> {
    await this.run("Pulling", () => pullRemote(repo, remote, ffOnly, (l) => (this.progress = l)));
  }

  async push(repo: RepoId, remote: string, force: boolean): Promise<void> {
    await this.run("Pushing", () => pushRemote(repo, remote, force, (l) => (this.progress = l)));
  }

  async run(label: string, operation: () => Promise<unknown>): Promise<void> {
    this.running = label;
    this.progress = null;
    try {
      await operation();
    } finally {
      this.running = null;
      this.progress = null;
    }
  }

  clear(): void {
    this.remotes = [];
    this.url = null;
    this.tokenStored = false;
    this.progress = null;
    this.running = null;
  }
}

export const network = new NetworkStore();
