import { authorsOf, keyOf, mergeRows, type Commitish } from "$lib/avatars";
import { avatarWindow, avatarsFor, setAvatars, type Author, type AvatarRow } from "$lib/ipc";

class AvatarStore {
  /** Off until asked for: turning it on is what creates the cache directory (M14 T14.3). */
  enabled = $state(false);
  rows = $state.raw<Map<string, AvatarRow>>(new Map());

  #window: Commitish[] = [];
  #queued: ReturnType<typeof setTimeout> | undefined;

  /** The window changes on every frame of a scroll. The queue only cares where the
      user stopped, so telling it that often is a round trip per frame for nothing. */
  static readonly #SETTLE_MS = 150;

  /** Follows the setting, which is the only thing that turns the cache directory on. */
  async apply(wanted: boolean): Promise<void> {
    if (wanted === this.enabled) return;
    this.enabled = wanted;
    try {
      await setAvatars(wanted);
    } catch {
      this.enabled = false;
      return;
    }
    if (wanted) await this.load(this.#window);
    else this.rows = new Map();
  }

  /** The rows on screen. The whole window goes to the queue every time — that is what
      lets it drop the rows that scrolled away — but only the authors this store has
      never seen are read back, because each one costs a file read and a base64 encode. */
  async load(window: readonly Commitish[]): Promise<void> {
    this.#window = [...window];
    if (!this.enabled) return;

    const authors = authorsOf(window);
    if (authors.length === 0) return;

    clearTimeout(this.#queued);
    this.#queued = setTimeout(() => void this.#settled(authors), AvatarStore.#SETTLE_MS);
  }

  async #settled(authors: readonly Author[]): Promise<void> {
    try {
      await avatarWindow(authors.map((author) => author.email));
      const missing = authors.filter((author) => !this.rows.has(keyOf(author.email)));
      if (missing.length === 0) return;
      this.rows = mergeRows(this.rows, await avatarsFor(missing));
    } catch {
      // A missing avatar is a fallback, not a dialog.
    }
  }

  /** One picture landed; only that author is asked for again. */
  async refresh(email: string): Promise<void> {
    if (!this.enabled) return;
    const author = this.#window.find((row) => keyOf(row.authorEmail) === keyOf(email));
    if (!author) return;
    try {
      this.rows = mergeRows(
        this.rows,
        await avatarsFor([{ name: author.authorName, email: author.authorEmail }]),
      );
    } catch {
      // Same: the row keeps its initials.
    }
  }

  look(email: string): AvatarRow | undefined {
    return this.rows.get(keyOf(email));
  }
}

export const avatars = new AvatarStore();
