import { keyLetter } from "$lib/key-letter";

export type DiffKey =
  | { kind: "jump"; by: 1 | -1 }
  | { kind: "layout" }
  | { kind: "find" }
  | { kind: "investigate" }
  | { kind: "close-find" };

export interface DiffPress {
  key: string;
  code?: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

export interface DiffKeyState {
  /** The diff has the keyboard: in the main window, the Diff panel has the focus. */
  active: boolean;
  findShowing: boolean;
  /** There is a change before, after the current one. */
  prev: boolean;
  next: boolean;
}

/** The keys of a diff (11 §7). They are its own only while it has the keyboard: F6 in Graph
    walked the panels and jumped in the diff at once, and Ctrl+F in the commit message pulled
    the focus into the diff's search. Past the last change F6 is the panel walk's again, so
    the keyboard can leave the panel. Only the layout toggle is global. */
export function diffKey(press: DiffPress, state: DiffKeyState): DiffKey | null {
  const letter = keyLetter(press);
  if (press.ctrl && press.shift && !press.alt && letter === "d") return { kind: "layout" };
  if (!state.active) return null;
  if (press.key === "F6" && !press.ctrl && !press.alt) {
    if (press.shift ? !state.prev : !state.next) return null;
    return { kind: "jump", by: press.shift ? -1 : 1 };
  }
  if (press.ctrl && !press.shift && !press.alt && letter === "f") return { kind: "find" };
  if (press.ctrl && press.alt && press.shift && letter === "l") return { kind: "investigate" };
  if (state.findShowing && press.key === "Escape") return { kind: "close-find" };
  return null;
}
