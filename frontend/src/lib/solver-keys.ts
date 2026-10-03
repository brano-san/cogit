import type { TakeAction } from "./solver-model";

export type SolverKey =
  | "save"
  | { step: "change" | "conflict"; by: 1 | -1 }
  | { take: TakeAction };

const TAKE_KEYS: Record<string, TakeAction> = {
  Digit1: "ours",
  Digit2: "theirs",
  Digit3: "oursTheirs",
  Digit4: "theirsOurs",
  Digit5: "base",
};

interface Press {
  key: string;
  code?: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

/** The keys of the Conflict Solver window (11 §9). Letters and digits by the place of the
    key: with another layout or with Shift the character is not the one printed there. */
export function solverKey(press: Press): SolverKey | null {
  if (!press.ctrl && !press.alt) {
    if (press.key === "F6") return { step: "change", by: press.shift ? -1 : 1 };
    if (press.key === "F7") return { step: "conflict", by: press.shift ? -1 : 1 };
    return null;
  }
  if (!press.ctrl || press.alt || press.shift) return null;
  if (press.code === "KeyS" || press.key === "s") return "save";
  const take = TAKE_KEYS[press.code ?? ""];
  return take ? { take } : null;
}
