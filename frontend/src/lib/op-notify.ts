import type { OperationKind } from "$lib/ipc";

/** Shorter than this, the user did not have time to look away: no notification. */
export const NOTIFY_AFTER_MS = 3000;

export type Outcome = "success" | "failure" | "attention";

export interface Finished {
  kind: OperationKind;
  label: string;
  repoName: string | null;
  outcome: Outcome;
  ms: number;
  /** What the in-app toast said, "Pushed 3 commits to origin/main". */
  summary: string | null;
}

export interface NotifyPrefs {
  enabled: boolean;
  success: boolean;
  failure: boolean;
}

export interface Shown {
  title: string;
  body: string;
  failed: boolean;
}

const NOUN: Partial<Record<OperationKind, string>> = {
  fetch: "Fetch",
  pull: "Pull",
  push: "Push",
  commit: "Commit",
  checkout: "Checkout",
  merge: "Merge",
  rebase: "Rebase",
  clone: "Clone",
  stash: "Stash",
  submodule: "Submodule update",
  worktree: "Worktree update",
  undo: "Undo",
};

/** What to tell the system, or null: in front, quick, or switched off says nothing. */
export function notificationFor(done: Finished, prefs: NotifyPrefs, focused: boolean): Shown | null {
  if (focused || !prefs.enabled || done.ms < NOTIFY_AFTER_MS) return null;
  const good = done.outcome === "success";
  if (good ? !prefs.success : !prefs.failure) return null;
  const what = NOUN[done.kind] ?? done.label;
  const verb = { success: "succeeded", failure: "failed", attention: "needs attention" }[done.outcome];
  const title = done.repoName ? `${what} ${verb} · ${done.repoName}` : `${what} ${verb}`;
  const body =
    done.outcome === "success"
      ? (done.summary ?? `${done.label} finished in ${Math.round(done.ms / 1000)} s.`)
      : done.outcome === "attention"
        ? "Conflicts are waiting to be resolved."
        : "Open Cogit for the full git output.";
  return { title, body, failed: !good };
}
