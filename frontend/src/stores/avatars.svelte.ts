import { authorsOf, keyOf, mergeRows, type Commitish } from "$lib/avatars";
import { avatarsFor, setAvatars, type AvatarRow } from "$lib/ipc";

class AvatarStore {
  /** Off until asked for: turning it on is what creates the cache directory (M14 T14.3). */
  enabled = $state(false);
  rows = $state.raw<Map<string, AvatarRow>>(new Map());

  #window: Commitish[] = [];

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

  /** The rows on screen. Sending the whole window each time is what lets the queue drop
      the requests for rows that have scrolled away. */
  async load(window: readonly Commitish[]): Promise<void> {
    this.#window = [...window];
    if (!this.enabled) return;

    const authors = authorsOf(window);
    if (authors.length === 0) return;
    try {
      this.rows = mergeRows(this.rows, await avatarsFor(authors));
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
