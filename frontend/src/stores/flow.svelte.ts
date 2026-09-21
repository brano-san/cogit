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

  async refresh(repo: RepoId): Promise<void> {
    try {
      this.status = await flowStatus(repo);
    } catch {
      this.status = EMPTY;
    }
  }

  async init(repo: RepoId): Promise<void> {
    await flowInit(repo, this.status.config);
    await this.refresh(repo);
  }

  async start(repo: RepoId, kind: FlowKind, name: string): Promise<void> {
    await flowStart(repo, kind, name);
    await this.refresh(repo);
  }

  async finish(repo: RepoId, kind: FlowKind, name: string, tag: string | null): Promise<void> {
    await flowFinish(repo, kind, name, tag);
    await this.refresh(repo);
  }

  clear(): void {
    this.status = EMPTY;
  }
}

export const flow = new FlowStore();
