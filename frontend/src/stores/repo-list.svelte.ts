import {
  forget,
  markClosed,
  markOpened,
  readRepoList,
  rename,
  togglePin,
  type RepoList,
} from "$lib/repo-list";

const STORAGE_KEY = "cogit.repo-list.v1";

function stored(): unknown {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

/** Closed rows, display names and pins of the Repositories list, next to its groups (#36). */
class RepoListStore {
  list = $state.raw<RepoList>(readRepoList(stored()));

  closed(root: string): void {
    this.write(markClosed(this.list, root));
  }

  opened(root: string): void {
    this.write(markOpened(this.list, root));
  }

  forget(root: string): void {
    this.write(forget(this.list, root));
  }

  rename(root: string, name: string): void {
    this.write(rename(this.list, root, name));
  }

  togglePin(root: string): void {
    this.write(togglePin(this.list, root));
  }

  private write(next: RepoList): void {
    if (next === this.list) return;
    this.list = next;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      // A blocked store costs the names and pins on restart, nothing more.
    }
  }
}

export const repoList = new RepoListStore();
