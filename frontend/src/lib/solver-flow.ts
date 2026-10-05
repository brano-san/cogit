import type { MergeToolOutcome } from "./ipc";
import { saveResolution } from "./merge-save";

export interface SaveSteps {
  /** Conflicts still undecided: saving them writes conflict markers. */
  unresolved: number;
  confirmMarkers: () => Promise<boolean>;
  write: () => Promise<unknown>;
  announce: () => Promise<unknown>;
  saved: () => void;
  close: () => Promise<unknown>;
}

export type SaveResult = { kind: "saved" } | { kind: "canceled" } | { kind: "failed"; message: string };

/** Save in the solver: the question about markers first, then written and staged in one
    step, announced to the main window, and the window closes behind itself. */
export async function saveFlow(steps: SaveSteps): Promise<SaveResult> {
  if (steps.unresolved > 0 && !(await steps.confirmMarkers())) return { kind: "canceled" };
  const failure = await saveResolution({
    resolve: steps.write,
    announce: steps.announce,
    saved: steps.saved,
    close: steps.close,
  });
  return failure === null ? { kind: "saved" } : { kind: "failed", message: failure };
}

export type CloseChoice = "save" | "discard" | "cancel";

/** What closing a still-conflicted file can do: stage it, leave it conflicted (saving the
    edits first), drop the edits, or stay. */
export type ResolveChoice = "resolve" | "keep" | "discard" | "cancel";

export interface ResolveClose {
  /** Left to right; the last is the primary one. */
  choices: { choice: ResolveChoice; label: string }[];
  /** What Enter does: Keep Unresolved while markers are in the Result, else Mark Resolved. */
  primary: ResolveChoice;
  warning: string | null;
}

/** One question when the solver closes on a file that is still conflicted, the unsaved
    edits folded into it rather than asked about in a second dialog (SmartGit's rule). */
export function resolveClose(dirty: boolean, markers: number): ResolveClose {
  const choices: ResolveClose["choices"] = [{ choice: "cancel", label: "Cancel" }];
  if (dirty) choices.push({ choice: "discard", label: "Discard Edits" });
  choices.push({ choice: "keep", label: dirty ? "Save, Keep Unresolved" : "Keep Unresolved" });
  choices.push({ choice: "resolve", label: dirty ? "Save and Mark Resolved" : "Mark Resolved" });
  const warning =
    markers > 0
      ? `${markers === 1 ? "1 conflict is" : `${markers} conflicts are`} still undecided: the file is written with conflict markers in place of ${markers === 1 ? "it" : "them"}.`
      : null;
  return { choices, primary: markers > 0 ? "keep" : "resolve", warning };
}

export type ToolFollowUp = "offerResolve" | "stillConflicted" | "canceled" | "alreadyResolved";

export function toolFollowUp(outcome: MergeToolOutcome): ToolFollowUp {
  if (outcome.canceled) return "canceled";
  if (!outcome.conflicted) return "alreadyResolved";
  return outcome.markersLeft ? "stillConflicted" : "offerResolve";
}

export function toolPrompt(path: string, outcome: MergeToolOutcome): string {
  const failed = outcome.exitCode !== null && outcome.exitCode !== 0;
  const code = failed ? ` The tool ended with exit code ${outcome.exitCode}.` : "";
  return `${path} has no conflict markers left.${code} Mark it resolved and stage it?`;
}
