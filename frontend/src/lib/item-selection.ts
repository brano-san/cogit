import { applyClick, type Modifiers } from "./multi-select";

/** The selection of a list that picks several rows (Graph, Branches): one model for both.
    `anchor` starts a Shift range, `cursor` is the end the arrows move. */
export interface ItemSelection {
  ids: ReadonlySet<string>;
  anchor: string | null;
  cursor: string | null;
}

export const NO_ITEMS: ItemSelection = { ids: new Set(), anchor: null, cursor: null };

export function single(id: string): ItemSelection {
  return { ids: new Set([id]), anchor: id, cursor: id };
}

/** Click, Ctrl+click and Shift+click. `order` is the rows from the anchor to `id` at least,
    in display order; a Shift range is cut from it. */
export function clickItem(
  current: ItemSelection,
  id: string,
  order: readonly string[],
  modifiers: Modifiers,
): ItemSelection {
  if (!order.includes(id)) return current;
  const next = applyClick({ paths: current.ids, anchor: current.anchor }, id, order, modifiers);
  return { ids: next.paths, anchor: next.anchor, cursor: id };
}

/** Shift+arrow, Shift+Page, Shift+Home/End: the range from the anchor to the row `by` rows
    from the cursor, clamped to the list. */
export function extendItems(current: ItemSelection, order: readonly string[], by: number): ItemSelection {
  if (order.length === 0) return current;
  const from = current.cursor === null ? -1 : order.indexOf(current.cursor);
  if (from < 0) return single(order[0] as string);
  const to = Math.min(Math.max(from + by, 0), order.length - 1);
  const anchor = current.anchor !== null && order.includes(current.anchor) ? current.anchor : (order[from] as string);
  const anchorAt = order.indexOf(anchor);
  const cursor = order[to] as string;
  return {
    ids: new Set(order.slice(Math.min(anchorAt, to), Math.max(anchorAt, to) + 1)),
    anchor,
    cursor,
  };
}

/** Ctrl+A: every visible row. The cursor stays where it was. */
export function selectEvery(current: ItemSelection, order: readonly string[]): ItemSelection {
  if (order.length === 0) return current;
  const cursor = current.cursor !== null && order.includes(current.cursor) ? current.cursor : (order[0] as string);
  return { ids: new Set(order), anchor: order[0] as string, cursor };
}

/** A right-click on a selected row keeps the group; on any other it replaces the selection. */
export function rightClickItem(current: ItemSelection, id: string): ItemSelection {
  return current.ids.has(id) ? current : single(id);
}

/** The selected rows in display order, whatever order they were clicked in. */
export function inOrder(current: ItemSelection, order: readonly string[]): string[] {
  return order.filter((id) => current.ids.has(id));
}

/** Rows that left the list (a filter, a fold, a reload) leave the selection. */
export function keepShown(current: ItemSelection, order: readonly string[]): ItemSelection {
  if (current.ids.size === 0) return current;
  const shown = new Set(order);
  const ids = [...current.ids].filter((id) => shown.has(id));
  if (ids.length === current.ids.size) return current;
  return {
    ids: new Set(ids),
    anchor: current.anchor !== null && shown.has(current.anchor) ? current.anchor : null,
    cursor: current.cursor !== null && shown.has(current.cursor) ? current.cursor : null,
  };
}
