export type OutputKey = "close" | "end-find" | "find" | "select-all" | "copy" | "bigger" | "smaller";

import { keyLetter } from "$lib/key-letter";

export interface OutputKeyPress {
  key: string;
  /** Where the key sits: a letter is read from it, whatever the layout types there. */
  code?: string;
  /** Ctrl, or ⌘ on macOS. */
  ctrl: boolean;
  /** Focus is inside the output window. */
  inside: boolean;
  /** Something that saw the key first already answered it. */
  handled: boolean;
  finding: boolean;
  allSelected: boolean;
}

/** The output window is not modal (R-89), so it answers only keys pressed inside it: a key
    typed anywhere else belongs to that field, and an Esc for a dialog is not its to take. */
export function outputKey(press: OutputKeyPress): OutputKey | null {
  if (!press.inside || press.handled) return null;
  if (press.key === "Escape") return press.finding ? "end-find" : "close";
  if (!press.ctrl) return null;
  switch (keyLetter(press)) {
    case "f":
      return "find";
    case "a":
      return "select-all";
    case "c":
      return press.allSelected ? "copy" : null;
    case "=":
    case "+":
      return "bigger";
    case "-":
      return "smaller";
    default:
      return null;
  }
}
