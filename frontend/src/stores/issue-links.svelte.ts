import { issueTrackers } from "$lib/ipc/remote-ops";
import { issueRule, type IssueRule } from "$lib/issue-links";
import type { RepoId } from "$lib/ipc";

/** The open repository's issue-link rule (F-953): read when it opens and after Repository ▸
    Settings is saved, not per commit. A failed read only means no links. */
class IssueLinksStore {
  rule = $state.raw<IssueRule | null>(null);
  #asked = 0;

  async load(repo: RepoId) {
    const ask = ++this.#asked;
    const trackers = await issueTrackers(repo).catch(() => []);
    if (ask === this.#asked) this.rule = issueRule(trackers);
  }

  clear() {
    this.#asked += 1;
    this.rule = null;
  }
}

export const issueLinks = new IssueLinksStore();
