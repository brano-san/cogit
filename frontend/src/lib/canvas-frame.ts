import { canvasPixelSize } from "$lib/graph-geometry";

/** The graph canvas's bitmap in device pixels and the CSS box that shows it. */
export interface CanvasBox {
  pixelWidth: number;
  pixelHeight: number;
  cssWidth: number;
  cssHeight: number;
}

/** The CSS box is the bitmap's, not the panel's: 401.6 px at 150 % is 602.4 device pixels, the
    bitmap has 603, and a 401.6 px box would squeeze it by a fraction that changes with every
    step of a resize (#27). The box may overhang the panel by less than a device pixel. */
export function canvasBox(width: number, height: number, dpr: number): CanvasBox {
  const ratio = dpr > 0 ? dpr : 1;
  const pixels = canvasPixelSize(width, height, ratio);
  return {
    pixelWidth: pixels.width,
    pixelHeight: pixels.height,
    cssWidth: pixels.width / ratio,
    cssHeight: pixels.height / ratio,
  };
}

/** A new box is drawn in the frame that resized it. Until the canvas is drawn again the
    browser paints the old bitmap stretched over the new box, so a draw a frame late moved
    every line against the rows beside it, on each step of a splitter drag (#27). Anything
    else waits for the next frame, where repeats collapse into one draw. */
export function drawsNow(drawn: CanvasBox | null, next: CanvasBox): boolean {
  return drawn === null || drawn.pixelWidth !== next.pixelWidth || drawn.pixelHeight !== next.pixelHeight;
}
