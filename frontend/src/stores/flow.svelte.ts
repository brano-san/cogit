import { flowFinish, flowInit, flowStart, flowStatus, type FlowKind, type FlowStatus, type RepoId } from "$lib/ipc";

const EMPTY: FlowStatus = {
  initialised: false,
  config: { main: "main", develop: "develop", feature: "feature/", release: "release/", hotfix: "hotfix/" },
  branches: [],
};

class FlowStore {
  status = $state.raw<FlowStatus>(EMPTY);

  /** The flow branch checked out right now, if the user is on one. */
  get current() {
    return this.status.branches.find((branch) => branch.isHead) ?? null;
  }

  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;
  /** Bumped by `clear()`: a write that finishes after it reads nothing back, since the
      panels it would read into belong to another repository by now. */
  #cleared = 0;

  async refresh(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    const status = await flowStatus(repo).catch(() => EMPTY);
    if (generation === this.#generation) this.status = status;
  }

  /** Read again before Finish: a checkout in Branches or a terminal moves HEAD off the
      branch the last read found, and Finish merges and deletes what it names. */
  async headNow(repo: RepoId) {
    await this.refresh(repo);
    return this.current;
  }

  async init(repo: RepoId): Promise<void> {
    const config = this.status.config;
    await this.#then(repo, () => flowInit(repo, config));
  }

  async start(repo: RepoId, kind: FlowKind, name: string): Promise<void> {
    await this.#then(repo, () => flowStart(repo, kind, name));
  }

  async finish(repo: RepoId, kind: FlowKind, name: string, tag: string | null): Promise<void> {
    await this.#then(repo, () => flowFinish(repo, kind, name, tag));
  }

  clear(): void {
    this.#generation += 1;
    this.#cleared += 1;
    this.status = EMPTY;
  }

  async #then(repo: RepoId, write: () => Promise<unknown>): Promise<void> {
    const cleared = this.#cleared;
    await write();
    if (cleared === this.#cleared) await this.refresh(repo);
  }
}

export const flow = new FlowStore();
