/** The UI font size the layout was drawn at: every row height and font token scales from it. */
export const BASE_FONT_PX = 13;

/** Diff rows stay 18 px (the diff, its editor and the Conflict Solver share one grid), so the
    diff font goes no larger than reads well in them. */
export const DIFF_FONT_PX = { default: 14, min: 10, max: 14 } as const;

export const UI_FONT_PX = { min: 11, max: 20 } as const;

/** The choices of Preferences, as `[value, label]`; 0 is the desktop's size. */
export function uiFontChoices(desktopPx: number | null): (readonly [string, string])[] {
  const sizes = [11, 12, 13, 14, 15, 16, 18, 20];
  return [["0", `System (${uiFontPx(0, desktopPx)} px)`] as const, ...sizes.map((size) => [String(size), `${size} px`] as const)];
}

/** The families of Preferences ▸ Font; "" is the desktop's. A chosen family the list lacks
    (uninstalled, or macOS, which lists none) stays, so the choice reads as it is. */
export function fontChoices(families: readonly string[], chosen: string): (readonly [string, string])[] {
  const listed = chosen && !families.includes(chosen) ? [chosen, ...families] : families;
  return [["", "System"] as const, ...listed.map((family) => [family, family] as const)];
}

export function diffFontChoices(): (readonly [string, string])[] {
  return [10, 11, 12, 12.5, 13, 14].map((size) => [String(size), `${size} px`] as const);
}

export function zoomChoices(): (readonly [string, string])[] {
  return ZOOM_STEPS.map((step) => [String(step), `${Math.round(step * 100)}%`] as const);
}

/** Ctrl+= and Ctrl+- step through these, as a browser does. */
export const ZOOM_STEPS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2] as const;

/** `setting` 0 follows the desktop (`gtk-font-name`, Segoe UI 9pt), never below the size the
    layout was drawn at: the UI read small next to other applications, never large. */
export function uiFontPx(setting: number, desktop: number | null): number {
  if (setting > 0) return Math.min(Math.max(setting, UI_FONT_PX.min), UI_FONT_PX.max);
  const system = desktop === null ? BASE_FONT_PX : Math.round(desktop * 2) / 2;
  return Math.min(Math.max(system, BASE_FONT_PX), UI_FONT_PX.max);
}

/** A length drawn for the base font, for `fontPx`. */
export function scaled(px: number, fontPx: number): number {
  return Math.round((px * fontPx) / BASE_FONT_PX);
}

export interface UiMetrics {
  fontPx: number;
  /** Files, Repositories, the default of every virtual list. */
  listRow: number;
  /** Branches, Repositories, Worktrees. */
  denseRow: number;
  /** The custom properties the stylesheet reads. */
  css: Record<string, string>;
}

export function uiMetrics(fontPx: number, diffPx: number): UiMetrics {
  const listRow = scaled(24, fontPx);
  const denseRow = scaled(22, fontPx);
  const px = (value: number) => `${value}px`;
  return {
    fontPx,
    listRow,
    denseRow,
    css: {
      "--fs-ui": px(fontPx),
      "--fs-dense": px(fontPx - 1),
      "--fs-status": px(fontPx - 1.5),
      "--fs-header": px(fontPx - 2),
      "--fs-small": px(fontPx - 2.5),
      "--fs-tiny": px(fontPx - 3),
      "--fs-code": px(Math.min(Math.max(diffPx, DIFF_FONT_PX.min), DIFF_FONT_PX.max)),
      "--lh-ui": px(scaled(20, fontPx)),
      "--lh-dense": px(scaled(18, fontPx)),
      "--h-row": px(listRow),
      "--h-row-dense": px(denseRow),
    },
  };
}

/** The next zoom step in `direction`, or 1 for 0; a value between steps goes to the nearest. */
export function zoomStep(current: number, direction: -1 | 0 | 1): number {
  if (direction === 0) return 1;
  const steps: readonly number[] = ZOOM_STEPS;
  const next =
    direction > 0 ? steps.find((step) => step > current + 0.001) : [...steps].reverse().find((step) => step < current - 0.001);
  return next ?? current;
}
