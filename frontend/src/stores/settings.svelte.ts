import { load } from "@tauri-apps/plugin-store";
import { formatCommitDate, relativeDate } from "$lib/format";
import { setLaneWidth } from "$lib/graph-geometry";
import { DEFAULT_DIFF_OPTIONS, type DiffOptions } from "$lib/ipc";
import { DEFAULT_SETTINGS, merge, type Settings } from "$lib/settings";

const FILE = "settings.json";
const KEY = "settings";

class SettingsStore {
  current = $state<Settings>({ ...DEFAULT_SETTINGS });

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
    return this.current.dateFormat === "relative"
      ? relativeDate(timestamp, offsetMinutes, Date.now() / 1000)
      : formatCommitDate(timestamp, offsetMinutes);
  }

  async load(): Promise<void> {
    try {
      const store = await load(FILE, { autoSave: false });
      this.current = merge(await store.get<Partial<Settings>>(KEY));
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
      const store = await load(FILE, { autoSave: false });
      await store.set(KEY, this.current);
      await store.save();
    } catch {
      // Unsaved is still applied: the change lasts for this session, not past a restart.
    }
  }

  async reset(): Promise<void> {
    this.current = { ...DEFAULT_SETTINGS };
    this.#apply();
    try {
      const store = await load(FILE, { autoSave: false });
      await store.set(KEY, this.current);
      await store.save();
    } catch {
      /* same */
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
