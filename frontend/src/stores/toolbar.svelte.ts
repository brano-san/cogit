import { isMergedIntoHead, type RepoId } from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import { DEFAULT_LAYOUT } from "$lib/toolbar";
import { normalizeLayout } from "$lib/toolbar-layout";
import { DEFAULT_PREFS, mergePrefs, type ToolbarPrefs } from "$lib/toolbar-prefs";

const KEY = "toolbar";
const MERGED_PAUSE_MS = 100;

class ToolbarStore {
  merged = $state<boolean | undefined>(undefined);
  prefs = $state.raw<ToolbarPrefs>({ ...DEFAULT_PREFS });
  layout = $state.raw<string[]>([...DEFAULT_LAYOUT]);
  #asked = 0;
  #pause: ReturnType<typeof setTimeout> | undefined;

  /**
   * Stale replies lose; a failed question offers the merge and lets Git say why. Asked after a
   * pause, so a key held down on the list sends one question, not one per row.
   */
  async checkMerged(repo: RepoId | undefined, commit: string | null, head: string | null) {
    const ticket = ++this.#asked;
    this.merged = undefined;
    clearTimeout(this.#pause);
    if (!repo || commit === null || commit === head) return;
    await new Promise<void>((resolve) => {
      this.#pause = setTimeout(resolve, MERGED_PAUSE_MS);
    });
    if (ticket !== this.#asked) return;
    const answer = await isMergedIntoHead(repo, commit).catch(() => false);
    if (ticket === this.#asked) this.merged = answer;
  }

  async load(): Promise<void> {
    try {
      const stored = await readKey<Record<string, unknown>>(KEY);
      this.prefs = mergePrefs(stored);
      this.layout = normalizeLayout(stored?.layout);
    } catch {
      this.prefs = { ...DEFAULT_PREFS };
      this.layout = [...DEFAULT_LAYOUT];
    }
  }

  async set<K extends keyof ToolbarPrefs>(key: K, value: ToolbarPrefs[K]): Promise<void> {
    this.prefs = mergePrefs({ ...this.prefs, [key]: value });
    await this.#save();
  }

  async setLayout(next: readonly string[]): Promise<void> {
    this.layout = normalizeLayout(next);
    await this.#save();
  }

  async #save(): Promise<void> {
    try {
      await writeKey(KEY, { ...this.prefs, layout: this.layout });
    } catch {
      // Unsaved is still applied: the choice lasts for this session, not past a restart.
    }
  }
}

export const toolbar = new ToolbarStore();
