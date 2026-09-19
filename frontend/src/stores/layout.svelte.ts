/**
 * Panel sizes for the main window.
 *
 * Sizes are stored as **fractions**, never pixels: a pixel layout saved on a 4K monitor
 * is unusable when the window is later opened on a laptop (doc/05-ui-layout.md section 2).
 *
 * Persistence currently uses `localStorage`, which survives restarts inside the webview.
 * M2 moves this to `tauri-plugin-store` so sizes can be scoped per perspective.
 */

const STORAGE_KEY = "cogit.layout.v1";

/** Neither side of a splitter may shrink past this share of its container. */
const MIN_FRACTION = 0.12;
const MAX_FRACTION = 1 - MIN_FRACTION;

export interface LayoutFractions {
  /** Left column (repositories + references) share of the window width. */
  leftColumn: number;
  /** Repositories share of the left column height. */
  repositories: number;
  /** Graph+files row share of the right area height. */
  topRow: number;
  /** Graph share of the top row width. */
  graph: number;
}

export const DEFAULT_LAYOUT: LayoutFractions = {
  leftColumn: 0.24,
  repositories: 0.45,
  topRow: 0.55,
  graph: 0.68,
};

/** Keeps a fraction inside the range where both panes stay usable. */
export function clampFraction(value: number): number {
  if (!Number.isFinite(value)) return MIN_FRACTION;
  return Math.min(MAX_FRACTION, Math.max(MIN_FRACTION, value));
}

function load(): LayoutFractions {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_LAYOUT };
    const parsed = JSON.parse(raw) as Partial<LayoutFractions>;
    return {
      leftColumn: clampFraction(parsed.leftColumn ?? DEFAULT_LAYOUT.leftColumn),
      repositories: clampFraction(parsed.repositories ?? DEFAULT_LAYOUT.repositories),
      topRow: clampFraction(parsed.topRow ?? DEFAULT_LAYOUT.topRow),
      graph: clampFraction(parsed.graph ?? DEFAULT_LAYOUT.graph),
    };
  } catch {
    // Corrupt or unavailable storage must never stop the app from opening.
    return { ...DEFAULT_LAYOUT };
  }
}

class LayoutStore {
  fractions = $state<LayoutFractions>(load());

  /** Adjusts one splitter by a signed fraction of its container. */
  nudge(key: keyof LayoutFractions, delta: number): void {
    this.fractions[key] = clampFraction(this.fractions[key] + delta);
    this.persist();
  }

  set(key: keyof LayoutFractions, value: number): void {
    this.fractions[key] = clampFraction(value);
    this.persist();
  }

  reset(): void {
    this.fractions = { ...DEFAULT_LAYOUT };
    this.persist();
  }

  resetOne(key: keyof LayoutFractions): void {
    this.fractions[key] = DEFAULT_LAYOUT[key];
    this.persist();
  }

  private persist(): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.fractions));
    } catch {
      // Storage being full or blocked is not worth interrupting the user over.
    }
  }
}

export const layout = new LayoutStore();
