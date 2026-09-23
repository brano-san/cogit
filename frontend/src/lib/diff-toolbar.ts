import type { EolInfo, Hunk, LineEnding } from "./ipc/bindings";

const ENDING: Record<LineEnding, string> = {
  lf: "LF",
  crlf: "CRLF",
  cr: "CR",
  mixed: "Mixed",
  none: "None",
};

/** A side with no lines in any hunk does not exist: the file was added or deleted. */
function absent(hunks: readonly Hunk[], side: "old" | "new"): boolean {
  if (hunks.length === 0) return false;
  return hunks.every((hunk) => (side === "old" ? hunk.oldLines : hunk.newLines) === 0);
}

/** A new file has only its new ending, a deleted one only its old ending (#17). */
export function eolLabel(eol: EolInfo, hunks: readonly Hunk[]): { text: string; title: string } {
  let text = `${ENDING[eol.old]} → ${ENDING[eol.new]}`;
  if (absent(hunks, "old")) text = ENDING[eol.new];
  else if (absent(hunks, "new")) text = ENDING[eol.old];
  return { text, title: `Line endings: ${text}` };
}

/** The button names the view it switches to; the tooltip says what that view is (#14). */
export function layoutTip(current: "split" | "unified"): string {
  const action =
    current === "split"
      ? "Switch to the unified view: both versions in one column"
      : "Switch to the side-by-side view: old and new versions in two columns";
  return `${action} (Ctrl+Shift+D)\nRemembered between runs`;
}
