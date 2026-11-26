import { readUserTheme } from "$lib/ipc";
import { setNativeTheme } from "$lib/ipc/file-menus";
import type { FixedTheme, Theme, ThemeAccents } from "$lib/settings";
import { applyAccent, applyVariables, cssVariables, isDarkTheme, isTheme, resolveTheme, THEME_CACHE_KEY } from "$lib/theme";
import { SYSTEM_THEME_QUERY } from "$lib/theme-cache";
import { resetTokenColors } from "$lib/theme-colors";
import { trace } from "$lib/trace";

/** Turns the chosen theme's JSON (plus `user-theme.json`) into custom properties on `:root`.
    Every window runs its own copy; `settings.#apply` calls it on load and on every change. */
class ThemeStore {
  /** Bumped by every swap: a canvas that read colors from the page redraws on it. */
  version = $state(0);
  resolved = $state<FixedTheme>("light");
  defaultAccent = $state(resolveTheme("light").tokens.accent!);
  #id: Theme = "system";
  #accents: ThemeAccents = {};
  #media: MediaQueryList | undefined;
  #systemChanged = () => this.#paint();
  #user: unknown = undefined;
  #warned = new Set<string>();
  #swaps = 0;

  apply(id: Theme, accents: ThemeAccents = {}): void {
    this.#id = id;
    this.#accents = { ...accents };
    if (id === "system" && typeof matchMedia === "function") {
      if (!this.#media) {
        this.#media = matchMedia(SYSTEM_THEME_QUERY);
        this.#media.addEventListener("change", this.#systemChanged);
      }
    } else {
      this.#media?.removeEventListener("change", this.#systemChanged);
      this.#media = undefined;
    }
    this.#paint();
  }

  #paint(): void {
    const choice = this.#id;
    const id = choice === "system" ? (this.#media?.matches ? "dark" : "light") : choice;
    const { tokens, warnings } = resolveTheme(id, this.#user);
    this.resolved = id;
    this.defaultAccent = tokens.accent!;
    const accent = this.#accents[id];
    if (accent !== undefined) applyAccent(tokens, accent, id);
    for (const warning of warnings) {
      if (this.#warned.has(warning)) continue;
      this.#warned.add(warning);
      trace("theme", warning, "warn");
    }
    if (typeof document === "undefined") return;
    applyVariables(document.documentElement, id, cssVariables(tokens));
    resetTokenColors();
    try {
      localStorage.setItem(THEME_CACHE_KEY, choice);
    } catch {
      // No storage: the first paint of the next start is the default theme's.
    }
    void Promise.resolve()
      .then(() => setNativeTheme(choice === "system" ? null : isDarkTheme(id), String(tokens["bg.app"] ?? "")))
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
    this.#paint();
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
  themeStore.apply(isTheme(cached) ? cached : "system");
}
