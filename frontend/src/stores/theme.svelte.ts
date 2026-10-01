import { readUserTheme } from "$lib/ipc";
import { setNativeTheme } from "$lib/ipc/file-menus";
import type { Theme } from "$lib/settings";
import { applyVariables, cssVariables, isDarkTheme, isTheme, resolveTheme, THEME_CACHE_KEY } from "$lib/theme";
import { resetTokenColors } from "$lib/theme-colors";
import { trace } from "$lib/trace";

/** Turns the chosen theme's JSON (plus `user-theme.json`) into custom properties on `:root`.
    Every window runs its own copy; `settings.#apply` calls it on load and on every change. */
class ThemeStore {
  /** Bumped by every swap: a canvas that read colors from the page redraws on it. */
  version = $state(0);
  #id: Theme = "dark";
  #user: unknown = undefined;
  #warned = new Set<string>();
  #swaps = 0;

  apply(id: Theme): void {
    this.#id = id;
    const { tokens, warnings } = resolveTheme(id, this.#user);
    for (const warning of warnings) {
      if (this.#warned.has(warning)) continue;
      this.#warned.add(warning);
      trace("theme", warning, "warn");
    }
    if (typeof document === "undefined") return;
    applyVariables(document.documentElement, id, cssVariables(tokens));
    resetTokenColors();
    try {
      localStorage.setItem(THEME_CACHE_KEY, id);
    } catch {
      // No storage: the first paint of the next start is the default theme's.
    }
    // GTK's file chooser and the like: dark for a dark family, whatever the system says.
    void Promise.resolve()
      .then(() => setNativeTheme(isDarkTheme(id)))
      .catch(() => {});
    // Written, never read: this runs inside effects, and a read would make them depend on it.
    this.version = ++this.#swaps;
  }

  /** Reads `user-theme.json` and applies it over the current theme. */
  async loadUser(): Promise<void> {
    let user: unknown;
    try {
      user = JSON.parse(await readUserTheme());
    } catch (err) {
      trace("theme", `user-theme.json unavailable: ${String(err)}`, "warn");
      user = undefined;
    }
    if (JSON.stringify(user) === JSON.stringify(this.#user)) return;
    this.#user = user;
    this.apply(this.#id);
  }
}

export const themeStore = new ThemeStore();

/** Before the first paint: the theme of the last run, so the window does not open in the
    wrong one while the settings file is still being read. */
export function bootTheme(): void {
  let cached: string | null = null;
  try {
    cached = localStorage.getItem(THEME_CACHE_KEY);
  } catch {
    // Treated as no cache.
  }
  themeStore.apply(isTheme(cached) ? cached : "dark");
}
