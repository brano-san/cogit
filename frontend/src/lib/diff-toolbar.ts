import type { EolInfo, LineEnding } from "./ipc/bindings";

const ENDING: Record<LineEnding, string> = {
  lf: "LF",
  crlf: "CRLF",
  cr: "CR",
  mixed: "Mixed",
  none: "None",
};

/** A new file has only its new ending, a deleted one only its old ending (#17). Told by
    the line counts: a hunk without context has an empty side in the middle of a file too. */
export function eolLabel(eol: EolInfo, oldTotal: number, newTotal: number): { text: string; title: string } {
  let text = `${ENDING[eol.old]} → ${ENDING[eol.new]}`;
  if (oldTotal === 0 && newTotal > 0) text = ENDING[eol.new];
  else if (newTotal === 0 && oldTotal > 0) text = ENDING[eol.old];
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
