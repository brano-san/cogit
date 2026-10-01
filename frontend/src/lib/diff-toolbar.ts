import type { EolInfo, LineEnding, Whitespace } from "./ipc/bindings";
import type { DiffLayout } from "./settings";

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
export function eolLabel(
  eol: EolInfo,
  oldTotal: number,
  newTotal: number,
): { text: string; title: string; warn: boolean } {
  let text = `${ENDING[eol.old]} → ${ENDING[eol.new]}`;
  if (oldTotal === 0 && newTotal > 0) text = ENDING[eol.new];
  else if (newTotal === 0 && oldTotal > 0) text = ENDING[eol.old];
  // Both sides exist and end their lines differently: the change rewrites every line.
  const warn = oldTotal > 0 && newTotal > 0 && eol.old !== eol.new;
  return {
    text,
    warn,
    title: warn
      ? `The line endings of this file change (${text}).`
      : `Line endings: ${text}`,
  };
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

const WHITESPACE: Record<
  Whitespace,
  { label: string; what: string; next: Whitespace }
> = {
  none: { label: "Whitespace: shown", what: "Every whitespace difference is shown", next: "trailing" },
  trailing: {
    label: "Ignore trailing ws",
    what: "Whitespace at the end of lines is ignored",
    next: "all",
  },
  all: { label: "Ignore all ws", what: "All whitespace differences are ignored", next: "none" },
};

/** The whitespace button of the diff bar: pressed while some whitespace is ignored. The
    indentation of a line is marked inline (`diff.*.word`) while it is not ignored (R-626). */
export function whitespaceButton(mode: Whitespace): {
  label: string;
  title: string;
  pressed: boolean;
  next: Whitespace;
  indent: boolean;
} {
  const { label, what, next } = WHITESPACE[mode];
  return {
    label,
    title: `${what}.\nClick for: ${WHITESPACE[next].label}`,
    pressed: mode !== "none",
    next,
    indent: mode !== "all",
  };
}

/** The label of the mode that shows everything, for the hint under a whitespace-only diff. */
export const WHITESPACE_SHOWN_LABEL = WHITESPACE.none.label;

/** The Aligned toggle of the diff bar. Pressed = Aligned (1:1, filler where a side has no
    lines); unpressed = Compact (each side its own rows, curved connectors). The tooltip says
    which one is on and what a click switches to. Unified has neither (R-628). */
export function alignedButton(layout: DiffLayout): {
  label: string;
  title: string;
  pressed: boolean;
  next: DiffLayout;
} {
  const aligned = layout === "aligned";
  return {
    label: "Aligned",
    pressed: aligned,
    next: aligned ? "compact" : "aligned",
    title: aligned
      ? "ON: Aligned 1:1. Both sides stay on the same rows; hatched filler where a side has no lines.\nClick for: Compact (each side its own lines, curved connectors)."
      : "OFF: Compact. Each side shows only its own lines, joined by curved connectors.\nClick for: Aligned 1:1 (same rows on both sides, hatched filler).\nSide by side only; Unified is not affected.",
  };
}
