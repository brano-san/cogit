import { untrack } from "svelte";
import { asCogitError } from "$lib/notices";
import { notices } from "$stores/notices.svelte";

/** After a dismissed error, the same one from the same source stays quiet this long: a
    refresh that fails again is not news, and a storm of them would never let the window close. */
const QUIET_MS = 10_000;

const lastShown = new Map<string, { text: string; at: number }>();

export const errors = {
  /** `source` (a path, a ref) names what the failing operation was about; with it, the same
      failure of the same operation on the same source is shown once while it is pending
      and not again for `QUIET_MS` after. Nothing else is dropped. Called from effects, so
      untracked: reading the queue it writes to made a dismissed error come straight back. */
  report(error: unknown, title: string, source?: string): void {
    untrack(() => {
      const text = asCogitError(error)?.message;
      if (source !== undefined && text !== undefined) {
        const key = `${title}\u0000${source}`;
        const now = Date.now();
        const seen = lastShown.get(key);
        const pending = notices.all.some(
          (notice) => notice.severity === "error" && notice.title === title && notice.body === text,
        );
        const repeat = seen?.text === text && (pending || now - seen.at < QUIET_MS);
        lastShown.set(key, { text, at: now });
        if (repeat) return;
      }
      notices.report(error, title);
    });
  },

  message(text: string, title: string): void {
    notices.message(text, title);
  },
};
