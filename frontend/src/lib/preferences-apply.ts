import type { DiffSpec, RepoId, Whitespace } from "./ipc";
import type { Keymap } from "./keymap";
import type { Settings } from "./settings";

/** Settings the open diff was computed with: changing one has to re-run it. */
const REDIFF: readonly (keyof Settings)[] = ["algorithm", "contextLines", "wordDiff", "detectMoves"];

export interface ApplyHost {
  current: () => Settings;
  apply: (next: Settings) => Promise<void>;
  setKeymap: (keymap: Keymap) => Promise<void>;
  /** A rebuilt menu bar starts with every tick cleared. */
  rebuiltMenu: () => void;
  repo: () => RepoId | null;
  diff: {
    whitespace: Whitespace;
    spec: DiffSpec | null;
    path: string | null;
    setWhitespace: (repo: RepoId, mode: Whitespace) => Promise<void>;
    load: (repo: RepoId, spec: DiffSpec, path: string) => Promise<void>;
  };
}

/** Every change in Preferences, and Cancel, which applies the settings of the opening
    the same way: what the open diff was computed with is computed again. */
export async function applyPreferences(next: Settings, keymap: Keymap, host: ApplyHost): Promise<void> {
  const before = host.current();
  const touched = (Object.keys(next) as (keyof Settings)[]).filter((key) => next[key] !== before[key]);
  await host.apply(next);
  await host.setKeymap(keymap);
  host.rebuiltMenu();

  const { diff } = host;
  const id = host.repo();
  const { spec, path } = diff;
  const open = id !== null && spec !== null && path !== null ? { id, spec, path } : null;
  if (touched.includes("ignoreWhitespace")) {
    const mode = host.current().ignoreWhitespace;
    // The next file opened reads the mode from the store, open file or not.
    if (open) await diff.setWhitespace(open.id, mode);
    else diff.whitespace = mode;
  } else if (open && touched.some((key) => REDIFF.includes(key))) {
    await diff.load(open.id, open.spec, open.path);
  }
}
