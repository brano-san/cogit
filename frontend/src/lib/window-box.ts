/** Where a floating window sits, in pixels from the top left of the app window. */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Viewport {
  width: number;
  height: number;
}

/** Below this the output block stops being worth reading, which is the whole point of
    the window. Taken from the brief. */
export const MIN_BOX = { w: 600, h: 400 } as const;

function fit(size: number, min: number, available: number): number {
  return Math.round(Math.min(Math.max(size, min), available));
}

export function clampBox(box: Box, viewport: Viewport): Box {
  const w = fit(box.w, MIN_BOX.w, viewport.width);
  const h = fit(box.h, MIN_BOX.h, viewport.height);
  return {
    w,
    h,
    x: Math.round(Math.min(Math.max(box.x, 0), Math.max(viewport.width - w, 0))),
    y: Math.round(Math.min(Math.max(box.y, 0), Math.max(viewport.height - h, 0))),
  };
}

/** Roughly two thirds of the window, centred: big enough to read a stack trace without
    covering the graph the reader is about to go back to. */
export function defaultBox(viewport: Viewport): Box {
  const w = fit(viewport.width * 0.66, MIN_BOX.w, viewport.width);
  const h = fit(viewport.height * 0.66, MIN_BOX.h, viewport.height);
  return clampBox({ w, h, x: (viewport.width - w) / 2, y: (viewport.height - h) / 2 }, viewport);
}
