export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface Placed {
  left: number;
  top: number;
  /** The menu scrolls past this instead of leaving the window. */
  maxHeight: number;
  above: boolean;
}

const MARGIN = 4;
const GAP = 2;

/** Right-aligned under `anchor`; flips above when the room below is smaller than the menu and
    the room above is larger; whatever still does not fit scrolls (`maxHeight`). Always
    inside the viewport. */
export function placePopup(
  anchor: Box,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): Placed {
  const below = viewport.height - anchor.bottom - GAP - MARGIN;
  const over = anchor.top - GAP - MARGIN;
  const above = size.height > below && over > below;
  const room = Math.max(0, above ? over : below);
  const height = Math.min(size.height, room);
  const width = Math.min(size.width, viewport.width - 2 * MARGIN);
  const left = Math.max(MARGIN, Math.min(anchor.right - width, viewport.width - MARGIN - width));
  const top = above ? anchor.top - GAP - height : anchor.bottom + GAP;
  return { left, top, maxHeight: room, above };
}
