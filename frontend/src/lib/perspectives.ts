const MIN_FRACTION = 0.12;
const MAX_FRACTION = 1 - MIN_FRACTION;

/** Fractions, not pixels: a layout saved on a 4K monitor is unusable on a laptop. */
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

export const PANELS = ["repositories", "refs", "graph", "files", "diff"] as const;
export type PanelId = (typeof PANELS)[number];

export interface Perspective {
  fractions: LayoutFractions;
  hidden: PanelId[];
}

export const PERSPECTIVE_IDS = ["main", "review"] as const;
export type PerspectiveId = (typeof PERSPECTIVE_IDS)[number];

export const DEFAULT_PERSPECTIVES: Record<PerspectiveId, Perspective> = {
  main: { fractions: { ...DEFAULT_LAYOUT }, hidden: [] },
  review: {
    fractions: { ...DEFAULT_LAYOUT, leftColumn: 0.18, topRow: 0.32 },
    hidden: ["repositories"],
  },
};

/** Maximizing overrides the perspective: Shift+F11 shows one panel whatever else is set. */
export function isVisible(
  perspective: Perspective,
  maximized: PanelId | null,
  panel: PanelId,
): boolean {
  if (maximized !== null) return maximized === panel;
  return !perspective.hidden.includes(panel);
}

/** Hiding the last visible panel would leave an empty window, so it is a no-op. */
export function toggleHidden(hidden: readonly PanelId[], panel: PanelId): PanelId[] {
  if (hidden.includes(panel)) return hidden.filter((item) => item !== panel);
  if (hidden.length + 1 >= PANELS.length) return [...hidden];
  return [...hidden, panel];
}

function isPanel(value: unknown): value is PanelId {
  return PANELS.includes(value as PanelId);
}

function fractions(stored: unknown): LayoutFractions {
  const source = (typeof stored === "object" && stored !== null ? stored : {}) as Record<
    string,
    unknown
  >;
  const merged = { ...DEFAULT_LAYOUT };
  for (const key of Object.keys(DEFAULT_LAYOUT) as (keyof LayoutFractions)[]) {
    if (typeof source[key] === "number") merged[key] = clampFraction(source[key]);
  }
  return merged;
}

function perspective(stored: unknown, fallback: Perspective): Perspective {
  if (typeof stored !== "object" || stored === null) return { ...fallback };
  const source = stored as { fractions?: unknown; hidden?: unknown };
  const hidden = Array.isArray(source.hidden) ? source.hidden.filter(isPanel) : fallback.hidden;
  return { fractions: fractions(source.fractions), hidden: [...hidden] };
}

/** Accepts v1 too — bare fractions — so an upgrade keeps the layout the user tuned. */
export function mergePerspectives(stored: unknown): Record<PerspectiveId, Perspective> {
  if (typeof stored !== "object" || stored === null) return structuredClone(DEFAULT_PERSPECTIVES);

  const source = stored as Record<string, unknown>;
  const legacy = PERSPECTIVE_IDS.every((id) => source[id] === undefined);
  if (legacy) {
    const upgraded = structuredClone(DEFAULT_PERSPECTIVES);
    upgraded.main.fractions = fractions(source);
    return upgraded;
  }

  return {
    main: perspective(source.main, DEFAULT_PERSPECTIVES.main),
    review: perspective(source.review, DEFAULT_PERSPECTIVES.review),
  };
}
