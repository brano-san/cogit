import { listRemotes, repoRefs, type RepoId } from "$lib/ipc";
import { currentRemote, headRemote } from "$lib/toolbar-prefs";

/** What a Fetch, Pull, Push or Sync lends from the app around it. */
export interface RunHost {
  epoch(): number;
  report(err: unknown, title: string): void;
  afterMutation(): Promise<void>;
}

/** Runs one network step. A failure is reported, and the panels are read again only while
    the repository the step started in is still on screen. Returns the epoch the step
    started in, or null when it failed. */
export async function runGuarded(
  host: RunHost,
  failure: string,
  step: () => Promise<unknown>,
): Promise<number | null> {
  const epoch = host.epoch();
  try {
    await step();
    return epoch;
  } catch (err) {
    host.report(err, failure);
    if (host.epoch() === epoch) await host.afterMutation();
    return null;
  }
}

/** The message and title for a repository with nowhere to fetch from or push to. */
export function noRemote(kind: "fetch" | "pull" | "push"): [message: string, title: string] {
  return ["This repository has no remote.", `Could not ${kind}`];
}

/** For a repository the panels may not show: the remote its HEAD branch tracks, else
    `origin`, else the first — what Pull in the toolbar uses for the one on screen. */
export async function trackedRemote(id: RepoId): Promise<string | null> {
  const [names, refs] = await Promise.all([
    listRemotes(id).catch(() => [] as string[]),
    repoRefs(id).catch(() => null),
  ]);
  return refs ? headRemote(refs, names) : currentRemote(null, names);
}
