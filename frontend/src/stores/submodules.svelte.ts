import { listSubmodules, updateSubmodule, type RepoId, type Submodule } from "$lib/ipc";

class SubmoduleStore {
  entries = $state.raw<Submodule[]>([]);

  async refresh(repo: RepoId): Promise<void> {
    this.entries = await listSubmodules(repo);
  }

  async update(repo: RepoId, path: string, init: boolean): Promise<void> {
    await updateSubmodule(repo, path, init);
    await this.refresh(repo);
  }

  clear(): void {
    this.entries = [];
  }
}

export const submodules = new SubmoduleStore();
