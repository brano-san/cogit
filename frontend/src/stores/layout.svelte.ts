/** Fractions, not pixels: a layout saved on a 4K monitor is unusable on a laptop. */

const STORAGE_KEY = "cogit.layout.v1";

const MIN_FRACTION = 0.12;
const MAX_FRACTION = 1 - MIN_FRACTION;

export interface LayoutFractions {
  leftColumn: number;
  repositories: number;
  topRow: number;
  graph: number;
}

export const DEFAULT_LAYOUT: LayoutFractions = {
  leftColumn: 0.24,
  repositories: 0.45,
  topRow: 0.55,
  graph: 0.68,
};

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
    return { ...DEFAULT_LAYOUT };
  }
}

class LayoutStore {
  fractions = $state<LayoutFractions>(load());

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
    }
  }
}

export const layout = new LayoutStore();
