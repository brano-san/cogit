import { readKey, writeKey } from "$lib/settings-file";
import { displayDate } from "$lib/format";
import { setLaneWidth } from "$lib/graph-geometry";
import { DEFAULT_DIFF_OPTIONS, type DiffOptions } from "$lib/ipc";
import { mergeKeymap, type Keymap } from "$lib/keymap";
import { defaultKeymap, setKeymap } from "$lib/ipc";
import { DEFAULT_SETTINGS, merge, type Settings } from "$lib/settings";

const KEY = "settings";
const KEYMAP_KEY = "keymap";

class SettingsStore {
  current = $state<Settings>({ ...DEFAULT_SETTINGS });
  /** Only the commands the user changed; the defaults live in Rust with the menu. */
  keymap = $state.raw<Keymap>({});
  bindings = $state.raw<import("$lib/ipc").KeyBinding[]>([]);

  get diffOptions(): DiffOptions {
    return {
      ...DEFAULT_DIFF_OPTIONS,
      algorithm: this.current.algorithm,
      contextLines: this.current.contextLines,
      wordDiff: this.current.wordDiff,
      detectMoves: this.current.detectMoves,
    };
  }

  formatDate(timestamp: number, offsetMinutes: number): string {
    return displayDate(timestamp, offsetMinutes, Date.now() / 1000, this.current.dateFormat);
  }

  async load(): Promise<void> {
    try {
      this.current = merge(await readKey<Partial<Settings>>(KEY));
      this.keymap = mergeKeymap(await readKey<unknown>(KEYMAP_KEY));
    } catch {
      // Unreadable store: a fresh install or a locked profile. Defaults still work.
      this.current = { ...DEFAULT_SETTINGS };
    }
    this.#apply();
  }

  async set<K extends keyof Settings>(key: K, value: Settings[K]): Promise<void> {
    this.current = merge({ ...this.current, [key]: value });
    this.#apply();
    try {
      await writeKey(KEY, this.current);
    } catch {
      // Unsaved is still applied: the change lasts for this session, not past a restart.
    }
  }

  async reset(): Promise<void> {
    this.current = { ...DEFAULT_SETTINGS };
    this.#apply();
    try {
      await writeKey(KEY, this.current);
    } catch {
      /* same */
    }
  }

  /** The defaults are Rust's, beside the menu they belong to; read once. */
  async loadBindings(): Promise<void> {
    try {
      this.bindings = await defaultKeymap();
    } catch {
      this.bindings = [];
    }
  }

  /** The menu bar is rebuilt from the new keys straight away; no restart needed. */
  async setKeymap(next: Keymap): Promise<void> {
    this.keymap = next;
    try {
      await setKeymap(next);
    } catch {
      // The stored map still wins on the next start, so the change is not lost.
    }
    try {
      await writeKey(KEYMAP_KEY, next);
    } catch {
      // Unsaved is still applied for this session.
    }
  }

  /** Writes a whole draft at once, so OK in the Preferences dialog is one save. */
  async apply(draft: Settings): Promise<void> {
    this.current = merge(draft);
    this.#apply();
    try {
      await writeKey(KEY, this.current);
    } catch {
      // Unsaved is still applied: the change lasts for this session, not past a restart.
    }
  }

  /** The two settings that live outside the reactive graph: CSS and canvas geometry. */
  #apply(): void {
    setLaneWidth(this.current.laneWidth);
    if (typeof document !== "undefined") {
      document.documentElement.dataset.theme = this.current.theme;
    }
  }
}

export const settings = new SettingsStore();
