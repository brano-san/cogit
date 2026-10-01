import { LAYOUTS, type SolverLayout } from "./solver-geometry";

export interface SolverPrefs {
  layout: SolverLayout;
  baseChanges: boolean;
}

export const DEFAULT_PREFS: SolverPrefs = { layout: "all", baseChanges: false };

const KEY = "cogit:solver";

/** Whatever was stored, mended: a key that is not what it should be falls back by itself. */
export function parsePrefs(raw: string | null): SolverPrefs {
  if (raw === null) return DEFAULT_PREFS;
  try {
    const stored = JSON.parse(raw) as Partial<Record<keyof SolverPrefs, unknown>> | null;
    const layout = LAYOUTS.find((each) => each.id === stored?.layout)?.id ?? DEFAULT_PREFS.layout;
    const baseChanges = typeof stored?.baseChanges === "boolean" ? stored.baseChanges : DEFAULT_PREFS.baseChanges;
    return { layout, baseChanges };
  } catch {
    return DEFAULT_PREFS;
  }
}

/** Per viewer and never essential: a blocked or cleared store only means the defaults. */
export function loadPrefs(storage: Pick<Storage, "getItem"> = localStorage): SolverPrefs {
  try {
    return parsePrefs(storage.getItem(KEY));
  } catch {
    return DEFAULT_PREFS;
  }
}

export function savePrefs(prefs: SolverPrefs, storage: Pick<Storage, "setItem"> = localStorage): void {
  try {
    storage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    // Private window or full store: the next window starts from the defaults.
  }
}
