export interface FileSelection {
  paths: ReadonlySet<string>;
  /** Where a shift-click measures its range from. */
  anchor: string | null;
}

export interface Modifiers {
  ctrl: boolean;
  shift: boolean;
}

export const EMPTY_SELECTION: FileSelection = { paths: new Set(), anchor: null };

export function applyClick(
  current: FileSelection,
  path: string,
  order: readonly string[],
  modifiers: Modifiers,
): FileSelection {
  const at = order.indexOf(path);
  if (at < 0) return current;

  if (modifiers.ctrl) {
    const paths = new Set(current.paths);
    if (!paths.delete(path)) paths.add(path);
    return { paths, anchor: path };
  }

  const anchorAt = current.anchor === null ? -1 : order.indexOf(current.anchor);
  if (modifiers.shift && anchorAt >= 0) {
    const from = Math.min(anchorAt, at);
    const to = Math.max(anchorAt, at);
    return { paths: new Set(order.slice(from, to + 1)), anchor: current.anchor };
  }

  return { paths: new Set([path]), anchor: path };
}

/** The shown file was let go — a re-clicked commit (#7) — so its marks go with it. Marks
    made while nothing was shown are kept: a ctrl-click marks without opening. */
export function afterDeselect(
  previous: string | null,
  next: string | null,
  marked: FileSelection,
): FileSelection {
  return previous !== null && next === null ? EMPTY_SELECTION : marked;
}

/** The marks an action may touch: only rows on screen. A file the filter hides or a commit
    took away stays marked for when it is back, but nothing acts on it unseen. */
export function shownMarks(marked: FileSelection, order: readonly string[]): FileSelection {
  if (marked.paths.size === 0) return marked;
  const shown = new Set(order);
  const paths = [...marked.paths].filter((path) => shown.has(path));
  if (paths.length === marked.paths.size) return marked;
  const anchor = marked.anchor !== null && shown.has(marked.anchor) ? marked.anchor : null;
  return { paths: new Set(paths), anchor };
}
