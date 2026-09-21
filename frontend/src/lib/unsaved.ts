export interface UnsavedState {
  /** The hook open in the editor, when its text differs from what is on disk. */
  hook: string | null;
  /** The file open in the merge view with a resolution that has not been written. */
  merge: string | null;
}

/** What would be lost by closing now, in a sentence, or null when nothing would be. */
export function unsavedSummary(state: UnsavedState): string | null {
  const parts: string[] = [];
  if (state.hook) parts.push(`the ${state.hook} hook`);
  if (state.merge) parts.push(`the resolution of ${state.merge}`);
  if (parts.length === 0) return null;
  return `${parts.join(" and ")} ${parts.length === 1 ? "has" : "have"} unsaved changes.`;
}
