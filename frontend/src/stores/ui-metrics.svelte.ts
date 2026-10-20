import { fontStacks, STACK_CANDIDATES } from "$lib/font-stack";
import { systemFont, type SystemFont } from "$lib/ipc";
import { OS } from "$lib/platform";
import { BASE_FONT_PX, DIFF_FONT_PX, uiFontPx, uiMetrics, type UiMetrics } from "$lib/ui-metrics";
import { trace } from "$lib/trace";
import { untrack } from "svelte";

/** The rows the readability report is about: graph, Files, Branches, diff. */
const SAMPLES = {
  "graph row": '[aria-label="Commits"] .row',
  "file row": ".file-list .row",
  "branch row": '[aria-label="References"] .row',
  "diff line": ".diff .line",
};

interface Sizes {
  uiFontSize: number;
  diffFontSize: number;
  uiZoom: number;
}

/** Font, row heights and zoom of this window, from Preferences and the desktop's font. Each
    window runs its own copy; `settings.#apply` calls it on load and on every change. */
class UiMetricsStore {
  current = $state.raw<UiMetrics>(uiMetrics(BASE_FONT_PX, DIFF_FONT_PX.default));
  #desktop = $state.raw<SystemFont | null>(null);
  #asked = false;
  #sizes: Sizes | null = null;
  #zoom = 1;

  /** Runs inside effects: what it reads of this store must not make them depend on it. */
  apply(sizes: Sizes): void {
    untrack(() => this.#applied(sizes));
  }

  #applied(sizes: Sizes): void {
    this.#sizes = sizes;
    if (!this.#asked) {
      this.#asked = true;
      void systemFont([...STACK_CANDIDATES]).then(
        (font) => {
          this.#desktop = font;
          if (this.#sizes) this.apply(this.#sizes);
        },
        (err) => trace("metrics", `desktop font unknown: ${String(err)}`, "warn"),
      );
    }
    const metrics = uiMetrics(uiFontPx(sizes.uiFontSize, this.#desktop?.sizePx ?? null), sizes.diffFontSize);
    this.current = metrics;
    if (typeof document === "undefined") return;
    const root = document.documentElement.style;
    for (const [name, value] of Object.entries(metrics.css)) root.setProperty(name, value);
    const stacks = fontStacks(OS, this.#desktop && { family: this.#desktop.family, installed: this.#desktop.installed });
    root.setProperty("--font-ui", stacks.ui);
    root.setProperty("--font-mono", stacks.mono);
    if (sizes.uiZoom !== this.#zoom) {
      this.#zoom = sizes.uiZoom;
      void import("@tauri-apps/api/webview")
        .then(({ getCurrentWebview }) => getCurrentWebview().setZoom(sizes.uiZoom))
        .catch((err) => trace("metrics", `zoom not set: ${String(err)}`, "warn"));
    }
  }

  /** Writes what the text renders at to `cogit.log`, once rows are on screen (or after 20 s
      with what there is), next to the backend's scale and desktop font lines (R-775). */
  logRendered(): void {
    if (typeof document === "undefined") return;
    let tries = 0;
    const look = () => {
      const found = Object.entries(SAMPLES).map(([name, selector]) => [name, document.querySelector(selector)] as const);
      if (found.some(([, el]) => !el) && ++tries < 20) return void setTimeout(look, 1000);
      trace("metrics", `devicePixelRatio=${window.devicePixelRatio} fontPx=${this.current.fontPx} listRow=${this.current.listRow}`);
      for (const [name, el] of found) {
        if (!el) {
          trace("metrics", `${name}: not on screen`);
          continue;
        }
        const css = getComputedStyle(el);
        trace("metrics", `${name}: ${css.fontSize}/${css.lineHeight} ${css.color} ${css.fontFamily} height=${el.getBoundingClientRect().height}`);
      }
    };
    look();
  }

  /** The desktop's own UI size, for the "System" choice in Preferences. */
  get desktopPx(): number | null {
    return this.#desktop?.sizePx ?? null;
  }

  get listRow(): number {
    return this.current.listRow;
  }

  get fontPx(): number {
    return this.current.fontPx;
  }
}

export const uiMetricsStore = new UiMetricsStore();
