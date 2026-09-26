import type { Operation, OperationChanged, OperationKind, RepoId } from "$lib/ipc";

/** How the exit was asked for: the window's ✕ or Alt+F4, the menu or its shortcut, or
    the system ending the session. */
export type ExitSource = "window" | "command" | "system";
export type ExitVariant = "plain" | "busy";
export type ExitAction = "cancel" | "exit" | "exitWhenDone" | "exitAnyway";

export interface ExitButton {
  action: ExitAction;
  label: string;
  tone: "normal" | "primary" | "warning";
  disabled?: boolean;
}

export interface ExitRow {
  id: number;
  text: string;
}

export interface NetworkLine {
  repo: RepoId | null;
  line: string | null;
}

export const DONT_SHOW_HINT = "You can turn this back on in Preferences → General";

/** "Don't show again" silences the question, never the warning that work would be lost. */
export function mustAskBeforeExit(
  confirmExit: boolean,
  unfinished: number,
  source: ExitSource,
): boolean {
  if (unfinished > 0) return true;
  return source !== "system" && confirmExit;
}

export function exitVariant(unfinished: number): ExitVariant {
  return unfinished > 0 ? "busy" : "plain";
}

export function exitHeading(variant: ExitVariant, unfinished: number): string {
  if (variant === "plain") return "Do you want to exit Cogit now?";
  return unfinished === 1
    ? "1 operation is still running"
    : `${unfinished} operations are still running`;
}

export function exitNote(source: ExitSource, variant: ExitVariant): string | null {
  if (variant !== "plain" || source !== "window") return null;
  return "By closing the last window you will exit Cogit.";
}

export function showsDontShow(variant: ExitVariant): boolean {
  return variant === "plain";
}

/** Left to right; the decision is rightmost, as in every dialog of the app. */
export function exitButtons(variant: ExitVariant, waiting: boolean): ExitButton[] {
  const cancel: ExitButton = { action: "cancel", label: "Cancel", tone: "normal" };
  if (variant === "plain") {
    return [cancel, { action: "exit", label: "Exit Now", tone: "primary" }];
  }
  return [
    cancel,
    waiting
      ? { action: "exitWhenDone", label: "Exiting When Done…", tone: "normal", disabled: true }
      : { action: "exitWhenDone", label: "Exit When Done", tone: "normal" },
    { action: "exitAnyway", label: "Exit Anyway", tone: "warning" },
  ];
}

/** Also what Enter does, wherever the focus is. */
export function defaultAction(variant: ExitVariant): ExitAction {
  return variant === "plain" ? "exit" : "cancel";
}

export function focusAfterSwitch(
  focused: ExitAction | null,
  variant: ExitVariant,
  waiting: boolean,
): ExitAction {
  const kept = exitButtons(variant, waiting).find(
    (button) => button.action === focused && !button.disabled,
  );
  return kept ? kept.action : defaultAction(variant);
}

/** The new `confirmExit`, or null to leave it alone. Only a box the user saw and then
    confirmed with Exit Now counts. */
export function confirmExitAfter(
  action: ExitAction,
  variant: ExitVariant,
  dontShowAgain: boolean,
  confirmExit: boolean,
): boolean | null {
  if (action !== "exit" || !showsDontShow(variant)) return null;
  return dontShowAgain === confirmExit ? !dontShowAgain : null;
}

export function progressPercent(line: string | null | undefined): number | undefined {
  const found = line?.match(/(\d{1,3})%/);
  if (!found) return undefined;
  return Math.min(100, Number(found[1]));
}

const KIND_TITLES: Record<OperationKind, string> = {
  fetch: "Fetch",
  pull: "Pull",
  push: "Push",
  commit: "Commit",
  checkout: "Checkout",
  branch: "Branch",
  merge: "Merge",
  rebase: "Rebase",
  stage: "Stage",
  discard: "Discard",
  stash: "Stash",
  tag: "Tag",
  worktree: "Worktree",
  submodule: "Submodule",
  undo: "Undo",
  clone: "Clone",
  other: "Operation",
};

const NETWORK: ReadonlySet<OperationKind> = new Set(["fetch", "pull", "push", "clone"]);

/** `Push · SignalGenerator200 · 45%`: kind, repository, and how far it got if git said. */
export function exitRows(
  pending: Iterable<Operation>,
  repoNames: ReadonlyMap<number, string>,
  network: NetworkLine,
): ExitRow[] {
  return [...pending].map((operation) => {
    const repo = operation.repo === null ? undefined : repoNames.get(operation.repo.valueOf());
    const parts = [KIND_TITLES[operation.kind]];
    if (repo !== undefined) parts.push(repo);
    parts.push(stateOf(operation, network));
    return { id: operation.id, text: parts.join(" · ") };
  });
}

function stateOf(operation: Operation, network: NetworkLine): string {
  if (operation.phase !== "running") return "waiting";
  // A clone has no repository yet, and neither has its line.
  const own =
    NETWORK.has(operation.kind) &&
    (operation.repo?.valueOf() ?? null) === (network.repo?.valueOf() ?? null);
  const percent = own ? progressPercent(network.line) : undefined;
  return percent === undefined ? "running" : `${percent}%`;
}

/** The snapshot is fetched asynchronously; the events heard meanwhile are replayed over it
    in order, so each operation ends in its latest state whichever came first. */
export function seedPending(
  snapshot: readonly Operation[],
  meanwhile: readonly OperationChanged[],
): Map<number, Operation> {
  const start = new Map(
    snapshot
      .filter((operation) => operation.phase !== "done")
      .map((operation) => [operation.id, operation]),
  );
  return meanwhile.reduce(applyPending, start);
}

export function applyPending(
  pending: ReadonlyMap<number, Operation>,
  event: OperationChanged,
): Map<number, Operation> {
  const next = new Map(pending);
  if (event.phase === "done") next.delete(event.id);
  else next.set(event.id, { ...event });
  return next;
}

export type Verdict = "wait" | "exit" | "stay";

/** A failure stops the wait: exiting would take the error message away with the window. */
export function waitVerdict(pending: number, failed: boolean): Verdict {
  if (failed) return "stay";
  return pending === 0 ? "exit" : "wait";
}

/** After the queue changed under an open dialog. A shutdown the app held is let go as
    soon as nothing would be lost by it; any other exit waits for the user. */
export function settleVerdict(
  source: ExitSource,
  waiting: boolean,
  pending: number,
  failed: boolean,
): Verdict {
  if (waiting) return waitVerdict(pending, failed);
  return source === "system" && pending === 0 ? "exit" : "wait";
}
