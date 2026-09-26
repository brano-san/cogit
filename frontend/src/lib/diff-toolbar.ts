import type { EolInfo, LineEnding } from "./ipc/bindings";

const ENDING: Record<LineEnding, string> = {
  lf: "LF",
  crlf: "CRLF",
  cr: "CR",
  mixed: "Mixed",
  none: "None",
};

/** `CRLF → LF`, as the toolbar and doc/08 §3 spell the endings. */
export function eolChangeText(from: LineEnding, to: LineEnding): string {
  return `${ENDING[from]} → ${ENDING[to]}`;
}

/** A new file has only its new ending, a deleted one only its old ending (#17). Told by
    the line counts: a hunk without context has an empty side in the middle of a file too. */
export function eolLabel(eol: EolInfo, oldTotal: number, newTotal: number): { text: string; title: string } {
  let text = `${ENDING[eol.old]} → ${ENDING[eol.new]}`;
  if (oldTotal === 0 && newTotal > 0) text = ENDING[eol.new];
  else if (newTotal === 0 && oldTotal > 0) text = ENDING[eol.old];
  return { text, title: `Line endings: ${text}` };
}

/** `100644 → 100755 (now executable)`: the modes as git prints them, and what the bit means. */
export function modeChangeText(oldMode: string, newMode: string): string {
  const executable = "100755";
  const what =
    newMode === executable ? " (now executable)" : oldMode === executable ? " (no longer executable)" : "";
  return `${oldMode} → ${newMode}${what}`;
}

/** The button names the view it switches to; the tooltip says what that view is (#14). */
export function layoutTip(current: "split" | "unified"): string {
  const action =
    current === "split"
      ? "Switch to the unified view: both versions in one column"
      : "Switch to the side-by-side view: old and new versions in two columns";
  return `${action} (Ctrl+Shift+D)\nRemembered between runs`;
}
