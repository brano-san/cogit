import type { MergeResolved, RepoId } from "./ipc";
import { failureText } from "./merge-params";

export interface SaveSteps {
  resolve: () => Promise<unknown>;
  /** `merge_resolved`: the event that tells the main window. */
  announce: () => Promise<unknown>;
  /** The window may close now without asking about unsaved picks. */
  saved: () => void;
  close: () => Promise<unknown>;
}

/** Save in the merge window: written, announced, and the window closes behind itself.
    The failure text when a step fails; the picks stay for another Save. */
export async function saveResolution(steps: SaveSteps): Promise<string | null> {
  try {
    await steps.resolve();
    await steps.announce();
    steps.saved();
    await steps.close();
    return null;
  } catch (err) {
    return failureText(err);
  }
}

export interface ResolvedDeps {
  /** The repository the panels show. */
  shown: () => RepoId | null;
  resolvedElsewhere: (path: string) => void;
  /** The file list and everything that hangs off it. */
  reload: () => Promise<void>;
}

/** The main window's side of `merge-resolved`. */
export async function answerMergeResolved(event: MergeResolved, deps: ResolvedDeps): Promise<void> {
  if (deps.shown()?.valueOf() !== event.repo.valueOf()) return;
  deps.resolvedElsewhere(event.path);
  await deps.reload();
}
