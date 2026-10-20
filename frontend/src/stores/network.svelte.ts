import {
  fetchRemote,
  listRemotes,
  pullRemote,
  pushRemote,
  remoteUrl,
  type PullOutcome,
  type Pushed,
  type RepoId,
  type SignIn,
} from "$lib/ipc";
import { STASH_AND_PULL } from "$lib/network-flow";
import { pullDoneText, pushDoneText } from "$lib/toolbar";
import { confirmation } from "$stores/confirm.svelte";
import { signedIn } from "$stores/sign-in.svelte";
import { successToast } from "$stores/success-toast.svelte";

/** `repo` is `null` for a clone: the repository does not exist until it ends. */
type Running = { id: number; repo: RepoId | null; label: string; progress: string | null };

class NetworkStore {
  remotes = $state.raw<string[]>([]);
  url = $state<string | null>(null);
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
  }

  async fetch(repo: RepoId, remote: string): Promise<void> {
    await this.run(repo, "Fetching", (onLine, credential) => fetchRemote(repo, remote, onLine, credential));
    successToast.show("Fetch succeeded");
  }

  /** On a detached HEAD the backend only fetches, and the toast says so (F-710). Local
      changes in the way of moving the branch are asked about, as the Pull dialog does. */
  async pull(repo: RepoId, remote: string, ffOnly: boolean): Promise<PullOutcome> {
    const run = (autostash: boolean) =>
      this.run(repo, "Pulling", (onLine, credential) =>
        pullRemote(repo, remote, ffOnly, autostash, onLine, credential),
      );
    let outcome = await run(false);
    if (outcome === "stashNeeded" && (await confirmation.ask(STASH_AND_PULL))) outcome = await run(true);
    if (outcome !== "stashNeeded") successToast.show(pullDoneText(outcome));
    return outcome;
  }

  /** `Pushed` on a detached HEAD: what went, empty when nothing could and nothing ran,
      and then no toast — the caller asks where HEAD goes (F-710). */
  async push(repo: RepoId, remote: string, force: boolean): Promise<Pushed | null> {
    const pushed = await this.run(repo, "Pushing", (onLine, credential) =>
      pushRemote(repo, remote, force, onLine, credential),
    );
    const text = pushDoneText(pushed);
    if (text !== null) successToast.show(text);
    return pushed;
  }

  /** Operations that ended in an error; the taskbar must not count them as finished well. */
  #failures = $state(0);

  get failures(): number {
    return this.#failures;
  }

  async run<T>(
    repo: RepoId | null,
    label: string,
    operation: (onLine: (line: string) => void, credential: SignIn | null) => Promise<T>,
  ): Promise<T> {
    const id = ++this.#started;
    this.#running = [...this.#running, { id, repo, label, progress: null }];
    // After `clear()` the operation is no longer listed, and its lines go nowhere.
    const onLine = (line: string) => {
      if (!this.#running.some((entry) => entry.id === id)) return;
      this.#running = this.#running.map((entry) => (entry.id === id ? { ...entry, progress: line } : entry));
    };
    try {
      // A refusal asks in the Sign in dialog and runs it again (F-082).
      return await signedIn((credential) => operation(onLine, credential));
    } catch (err) {
      this.#failures += 1;
      throw err;
    } finally {
      this.#running = this.#running.filter((entry) => entry.id !== id);
    }
  }

  /** A clone stays: it belongs to none of the repositories the panels leave. */
  clear(): void {
    this.#generation += 1;
    this.remotes = [];
    this.url = null;
    this.#running = this.#running.filter((entry) => entry.repo === null);
  }
}

export const network = new NetworkStore();
