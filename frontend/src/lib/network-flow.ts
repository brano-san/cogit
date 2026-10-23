/** What follows the Pull and Push dialogs' git commands when notes are involved: a
    divergence is asked about, never settled silently. */
import type { ConfirmRequest } from "$stores/confirm.svelte";
import type { FetchOptions, NotesFetch, PullOptions, Pulled, PushOptions, PushOutcome } from "$lib/ipc/network-dialogs";

export interface NetworkApi {
  pullWith(repo: number, remote: string, options: PullOptions): Promise<Pulled>;
  fetchWith(repo: number, remote: string, options: FetchOptions): Promise<NotesFetch>;
  pushWith(repo: number, options: PushOptions): Promise<PushOutcome>;
  pushNotes(repo: number, remote: string): Promise<PushOutcome>;
  mergeNotes(repo: number, remote: string, namespace: string): Promise<void>;
}

export interface FlowUi {
  ask(request: ConfirmRequest): Promise<boolean>;
  /** A refusal git worded itself, shown whole. */
  report(message: string, title: string): void;
}

async function mergeAll(api: NetworkApi, repo: number, fetched: NotesFetch): Promise<void> {
  for (const namespace of fetched.diverged) await api.mergeNotes(repo, fetched.remote, namespace);
}

/** Fetched notes that diverged from ours: `git notes merge` on a yes; on a no they stay
    beside ours in `refs/notes-remote/<remote>/`. */
export async function resolveDivergedNotes(
  api: NetworkApi,
  ui: FlowUi,
  repo: number,
  fetched: NotesFetch,
): Promise<boolean> {
  if (fetched.diverged.length === 0) return false;
  const yes = await ui.ask({
    title: "Notes have diverged",
    message:
      `Local notes and the notes of ${fetched.remote} both changed. Yours were not overwritten; ` +
      "the remote copy is kept in refs/notes-remote. Merge them with git notes merge?",
    confirm: "Merge notes",
    items: fetched.diverged.map((name) => `refs/notes/${name}`),
  });
  if (yes) await mergeAll(api, repo, fetched);
  return yes;
}

/** A pull that has to move the branch found local changes and only fetched (`stashNeeded`):
    git's `--autostash` stashes them, pulls and applies them back on a yes. */
export const STASH_AND_PULL: ConfirmRequest = {
  title: "Local changes in the way",
  message:
    "Pulling moves the current branch, and the working tree has local changes. Stash them, pull, " +
    "and apply them back? If they conflict with what arrives, they stay safe in the stash.",
  confirm: "Stash and Pull",
};

/** What the pull did; `null` after Fetch Only, or when the local changes stayed and nothing
    was pulled, so there is no success to show. */
export async function pullFlow(
  api: NetworkApi,
  ui: FlowUi,
  repo: number,
  remote: string,
  options: PullOptions | { fetchOnly: FetchOptions },
): Promise<Pulled | null> {
  if ("fetchOnly" in options) {
    await resolveDivergedNotes(api, ui, repo, await api.fetchWith(repo, remote, options.fetchOnly));
    return null;
  }
  let pulled = await api.pullWith(repo, remote, options);
  if (pulled.stashNeeded) {
    if (!(await ui.ask(STASH_AND_PULL))) return null;
    pulled = await api.pullWith(repo, remote, { ...options, autostash: true });
  }
  await resolveDivergedNotes(api, ui, repo, pulled.notes);
  return pulled;
}

/** The branch, then the notes. A remote that moved its notes gets one offer: fetch,
    merge, send them again. */
export async function pushFlow(api: NetworkApi, ui: FlowUi, repo: number, options: PushOptions): Promise<void> {
  const outcome = await api.pushWith(repo, options);
  if (outcome.notesRejected === null) return;
  const yes = await ui.ask({
    title: "Notes were rejected",
    message:
      `${options.remote} has notes you do not have, so it refused yours. The branch was pushed. ` +
      `Fetch its notes, merge them with git notes merge and push again?\n\n${outcome.notesRejected}`,
    confirm: "Fetch and merge",
  });
  if (!yes) return;
  const fetched = await api.fetchWith(repo, options.remote, { tags: false, notes: true });
  await mergeAll(api, repo, fetched);
  const again = await api.pushNotes(repo, options.remote);
  if (again.notesRejected !== null) ui.report(again.notesRejected, "Could not push notes");
}
