export interface UnsavedState {
  /** The hook open in the editor, when its text differs from what is on disk. */
  hook: string | null;
  /** The file open in the merge view with a resolution that has not been written. */
  merge: string | null;
  /** Titles of the dialogs holding typed work (`ModalStack.unsaved`). */
  dialogs?: readonly string[];
}

/** What would be lost by closing now, in a sentence, or null when nothing would be. */
export function unsavedSummary(state: UnsavedState): string | null {
  const parts: string[] = [];
  if (state.hook) parts.push(`the ${state.hook} hook`);
  if (state.merge) parts.push(`the resolution of ${state.merge}`);
  for (const title of state.dialogs ?? []) parts.push(`“${title}”`);
  if (parts.length === 0) return null;
  return `${parts.join(" and ")} ${parts.length === 1 ? "has" : "have"} unsaved changes.`;
}

/** What asked a dialog to close. */
export type CloseRequest = "scrim" | "escape" | "button";

/** A dialog holding typed work is not closed by a stray click beside it, and Esc or the ✕
    ask before the work goes (R-515). */
export function closeAnswer(request: CloseRequest, dirty: boolean): "close" | "ask" | "stay" {
  if (!dirty) return "close";
  return request === "scrim" ? "stay" : "ask";
}
