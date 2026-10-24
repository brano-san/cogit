import type { ErrorEntry, ErrorOffer, GitOutput } from "$lib/ipc";

export type { ErrorEntry };
export { repoNameOf } from "$lib/notices";

/** What a finished command tells: enough to title it and to find its record again. */
type Run = Pick<
  GitOutput,
  "id" | "repo" | "operation" | "command" | "summary" | "stoppedOnConflicts" | "expected"
>;

/** The output of an entry: what the journal has, else what was received while it had it,
    else `"gone"`. Never a record made up from the entry: the journal keeps 100 commands. */
export function outputToShow(found: GitOutput | null, cached: GitOutput | undefined): GitOutput | "gone" {
  return found ?? cached ?? "gone";
}

/** The window's title names what it holds: a merge that stopped on conflicts is a warning,
    not an error. */
export function windowTitle(entries: readonly Pick<ErrorEntry, "kind">[]): string {
  const errors = entries.some((entry) => entry.kind === "error");
  const warnings = entries.some((entry) => entry.kind === "warning");
  const what = errors && warnings ? "Errors and Warnings" : errors ? "Errors" : entries.length > 1 ? "Warnings" : "Warning";
  return `${what} — Cogit`;
}

/** What did not work; a command that went as far as it could and left conflicts, or failed
    for an ordinary state of things (R-840), is named for what happened, not for failing. */
export function titleOf(run: Run): string {
  if (run.expected === "upstreamGone") return "Upstream branch is gone";
  if (run.expected === "pushBehind") return "Push rejected";
  if (run.expected === "submodulesFailed") return "Submodules not cloned";
  if (!run.stoppedOnConflicts) return `${run.operation} failed`;
  if (/\bgit\b.*\bstash\b/.test(run.command)) return "Stash applied with conflicts";
  if (/\bgit\b.*\bpull\b.*\s--rebase\b/.test(run.command)) return "Rebase stopped with conflicts";
  return `${run.operation} stopped with conflicts`;
}

function offerOf(run: Run): ErrorOffer | null {
  if (run.stoppedOnConflicts) return "showConflicts";
  return run.expected === "pushBehind" ? "pull" : null;
}

export function entryOf(run: Run): ErrorEntry {
  return {
    id: run.id,
    kind: run.stoppedOnConflicts || run.expected !== null ? "warning" : "error",
    title: titleOf(run),
    operation: run.operation,
    repo: run.repo,
    command: run.command,
    summary: run.summary,
    offer: offerOf(run),
    repeats: 1,
  };
}

/** Newest first, errors before warnings. One record is listed once, and the same failure
    again (same title, repository, command and words) is a count on the entry already
    there, moved to the front. */
export function pushEntry(list: readonly ErrorEntry[], entry: ErrorEntry): ErrorEntry[] {
  if (list.some((held) => held.id === entry.id)) return [...list];
  const same = list.find(
    (held) =>
      held.kind === entry.kind &&
      held.title === entry.title &&
      held.repo === entry.repo &&
      held.command === entry.command &&
      held.summary === entry.summary,
  );
  const rest = same ? list.filter((held) => held !== same) : [...list];
  const next = { ...entry, repeats: (same?.repeats ?? 0) + 1 };
  const errors = rest.filter((held) => held.kind === "error");
  const warnings = rest.filter((held) => held.kind === "warning");
  return next.kind === "error" ? [next, ...errors, ...warnings] : [...errors, next, ...warnings];
}

interface Press {
  button: number;
  target: EventTarget | { closest(selector: string): unknown } | null;
}

/** A header press starts a drag, unless it lands on a button in the header: the drag
    captures the pointer, which turns the button's click into the header's. */
export function startsDrag(press: Press): boolean {
  const target = press.target as { closest?: (selector: string) => unknown } | null;
  return press.button === 0 && !target?.closest?.("button");
}
