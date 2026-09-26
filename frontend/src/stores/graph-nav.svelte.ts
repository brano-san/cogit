import { SelectionHistory, type Selected } from "$lib/selection-history";
import type { RepoId } from "$lib/ipc";

/** Home and Back beside the graph filter (F-562): one history of selections per repository. */
class GraphNavStore {
  #histories = new Map<RepoId, SelectionHistory>();
  #repo: RepoId | null = null;
  canGoBack = $state(false);
  /** Bumped to take the list to its top, where the Working Tree row is. */
  top = $state(0);

  /** The graph of `repo` now has `selected`. */
  track(repo: RepoId | null, selected: Selected): void {
    this.#repo = repo;
    if (repo === null) {
      this.canGoBack = false;
      return;
    }
    let history = this.#histories.get(repo);
    if (!history) {
      history = new SelectionHistory();
      this.#histories.set(repo, history);
    }
    history.visit(selected);
    this.canGoBack = history.canGoBack;
  }

  /** Where Back goes, already taken off the history; `undefined` when there is nowhere. */
  back(): Selected | undefined {
    const history = this.#repo === null ? undefined : this.#histories.get(this.#repo);
    const target = history?.back();
    this.canGoBack = history?.canGoBack ?? false;
    return target;
  }

  toTop(): void {
    this.top += 1;
  }
}

export const graphNav = new GraphNavStore();
