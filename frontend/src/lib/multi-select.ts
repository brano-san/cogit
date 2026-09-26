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

/** A row of a list in sections. A partly staged file is in Unstaged and in Staged: two rows,
    selected and ticked apart, a Shift range kept to its own section. */
export function rowKey(section: number, path: string): string {
  return `${section}:${path}`;
}

export function rowOf(key: string): { section: number; path: string } {
  const cut = key.indexOf(":");
  return { section: Number(key.slice(0, cut)), path: key.slice(cut + 1) };
}

/** The ticked paths of each section, and every ticked path once, for what acts on paths. */
export function markedRows(keys: Iterable<string>): { bySection: Map<number, Set<string>>; paths: string[] } {
  const bySection = new Map<number, Set<string>>();
  const paths = new Set<string>();
  for (const key of keys) {
    const { section, path } = rowOf(key);
    const set = bySection.get(section) ?? new Set<string>();
    set.add(path);
    bySection.set(section, set);
    paths.add(path);
  }
  return { bySection, paths: [...paths] };
}

/** Said by the button, not guessed from how many paths came: a heading listing one row
    asks for that row, not for the marks. */
export type ActionRequest = { row: string } | { all: readonly string[] };

/** A row's button acts on the section's marked rows when it is one of them; a heading's
    "all" acts on the rows it lists. */
export function actionScope(marked: ReadonlySet<string>, request: ActionRequest): string[] {
  if ("all" in request) return [...request.all];
  return marked.has(request.row) && marked.size > 1 ? [...marked] : [request.row];
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

/** The Repositories list: a plain click opens the row and ticks nothing, so the ticks are
    only the rows picked on purpose; it still anchors a later Shift range. */
export function markRow(
  current: FileSelection,
  path: string,
  order: readonly string[],
  modifiers: Modifiers,
): FileSelection {
  if (modifiers.ctrl || modifiers.shift) return applyClick(current, path, order, modifiers);
  return order.includes(path) ? { paths: new Set(), anchor: path } : current;
}

/** Why a row's button is off: the first file of those it would act on (`actionScope`) that
    the rule refuses, so the button agrees with the menu for the same marks. */
export function scopeBlocked<F>(
  marked: ReadonlySet<string>,
  row: string,
  files: ReadonlyMap<string, F>,
  blocked: (file: F) => string | null,
): string | null {
  return requestBlocked(marked, { row }, files, blocked);
}

/** For an action that passes over the files it does not apply to: off only when it applies
    to none of `paths`, with the first one's reason. */
export function everyBlocked<F>(
  paths: readonly string[],
  files: ReadonlyMap<string, F>,
  blocked: (file: F) => string | null,
): string | null {
  let first: string | null = null;
  for (const path of paths) {
    const file = files.get(path);
    const reason = file === undefined ? null : blocked(file);
    if (reason === null) return null;
    first ??= reason;
  }
  return first;
}

/** The same for any button: a row's, or a heading's "all" over the rows it lists. */
export function requestBlocked<F>(
  marked: ReadonlySet<string>,
  request: ActionRequest,
  files: ReadonlyMap<string, F>,
  blocked: (file: F) => string | null,
): string | null {
  for (const path of actionScope(marked, request)) {
    const file = files.get(path);
    const reason = file === undefined ? null : blocked(file);
    if (reason !== null) return reason;
  }
  return null;
}
