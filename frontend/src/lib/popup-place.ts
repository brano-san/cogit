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

/** A context menu: its corner at the pointer, flipped to the left or above it where the
    window ends, scrolling past what still does not fit. Always inside the viewport. */
export function placeAtPoint(
  point: { x: number; y: number },
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number; maxHeight: number } {
  const width = Math.min(size.width, viewport.width - 2 * MARGIN);
  const left = Math.max(
    MARGIN,
    point.x + width > viewport.width - MARGIN ? point.x - width : point.x,
  );
  const below = viewport.height - point.y - MARGIN;
  const over = point.y - MARGIN;
  const above = size.height > below && over > below;
  const room = Math.max(0, above ? over : below);
  const height = Math.min(size.height, room);
  const top = Math.max(MARGIN, above ? point.y - height : point.y);
  return { left: Math.min(left, viewport.width - MARGIN - width), top, maxHeight: room };
}

const SUBMENU_OVERLAP = 1;
const SUBMENU_LIFT = 4;

/** A submenu: right of its row, one pixel over the parent's edge and level with the first
    item; on the left when the right has no room and the left has more; slid up where the
    window ends. */
export function placeSubmenu(
  row: Box,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number; maxHeight: number; flipped: boolean } {
  const width = Math.min(size.width, viewport.width - 2 * MARGIN);
  const roomRight = viewport.width - MARGIN - (row.right - SUBMENU_OVERLAP);
  const roomLeft = row.left + SUBMENU_OVERLAP - MARGIN;
  const flipped = width > roomRight && roomLeft > roomRight;
  const left = flipped ? row.left + SUBMENU_OVERLAP - width : row.right - SUBMENU_OVERLAP;
  const maxHeight = viewport.height - 2 * MARGIN;
  const height = Math.min(size.height, maxHeight);
  const top = Math.max(MARGIN, Math.min(row.top - SUBMENU_LIFT, viewport.height - MARGIN - height));
  return { left: Math.max(MARGIN, left), top, maxHeight, flipped };
}

/** A drop-down of the menu bar: left edges aligned with its title, directly below it,
    shifted left where the window ends, scrolling instead of flipping above. */
export function placeDropdown(
  anchor: Box,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number; maxHeight: number } {
  const width = Math.min(size.width, viewport.width - 2 * MARGIN);
  const left = Math.max(MARGIN, Math.min(anchor.left, viewport.width - MARGIN - width));
  return { left, top: anchor.bottom, maxHeight: Math.max(0, viewport.height - anchor.bottom - MARGIN) };
}
