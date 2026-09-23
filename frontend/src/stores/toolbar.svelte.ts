import { isMergedIntoHead, type RepoId } from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import { DEFAULT_LAYOUT } from "$lib/toolbar";
import { normalizeLayout } from "$lib/toolbar-layout";
import { DEFAULT_PREFS, mergePrefs, type ToolbarPrefs } from "$lib/toolbar-prefs";

const KEY = "toolbar";

/** What the button toolbar keeps between renders beyond the shared stores. */
class ToolbarStore {
  /** Whether HEAD already contains the selected commit; `undefined` until answered. */
  merged = $state<boolean | undefined>(undefined);
  prefs = $state.raw<ToolbarPrefs>({ ...DEFAULT_PREFS });
  /** Button ids and separators in order (#44). */
  layout = $state.raw<string[]>([...DEFAULT_LAYOUT]);
  configuring = $state(false);
  #asked = 0;

  /** Clicking down the graph asks faster than the backend answers; stale replies lose. */
  async checkMerged(repo: RepoId | undefined, commit: string | null, head: string | null) {
    const ticket = ++this.#asked;
    this.merged = undefined;
    if (!repo || commit === null || commit === head) return;
    // A failed question offers the merge: Git then says why, rather than a button that
    // stays grey with no reason.
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
