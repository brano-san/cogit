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

/** The window is closing with edits: only Discard lets it. Save closes it itself, once written. */
export function afterCloseChoice(choice: CloseChoice): { close: boolean; save: boolean } {
  return { close: choice === "discard", save: choice === "save" };
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
