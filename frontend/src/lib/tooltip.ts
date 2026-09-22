/** One delay for every tooltip in the app, native `title` ones included (R-181). */
export const TIP_DELAY_MS = 400;

const GAP = 6;
const MARGIN = 4;

export interface Box {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface Placement {
  left: number;
  top: number;
  below: boolean;
}

export function placeTip(
  anchor: Box,
  tip: { width: number; height: number },
  viewport: { width: number; height: number },
  preferBelow = false,
): Placement {
  const roomAbove = anchor.top - GAP - tip.height >= MARGIN;
  const roomBelow = anchor.top + anchor.height + GAP + tip.height <= viewport.height - MARGIN;
  const below = preferBelow ? roomBelow || !roomAbove : !roomAbove && roomBelow;

  const centred = anchor.left + anchor.width / 2 - tip.width / 2;
  const left = Math.max(MARGIN, Math.min(centred, viewport.width - MARGIN - tip.width));
  const top = below ? anchor.top + anchor.height + GAP : anchor.top - GAP - tip.height;
  return { left, top: Math.max(MARGIN, top), below };
}
