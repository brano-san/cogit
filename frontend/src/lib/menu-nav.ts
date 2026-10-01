import { menuKey } from "./menu-keys";

/** A row as the page draws it, from a context menu or from the menu bar's model. */
export interface MenuRow {
  id: string;
  label: string;
  enabled: boolean;
  separator: boolean;
  /** Shown on the right, already formatted. */
  shortcut: string | null;
  /** `null` is a plain row, a boolean a toggle with its tick. */
  checked: boolean | null;
  children: MenuRow[];
}

/** The focused row at each open level; a level past the first exists while its submenu is
    open, and `-1` is "open, nothing focused yet" (the pointer opened it). */
export interface Nav {
  path: number[];
}

export type NavResult =
  | { kind: "state"; nav: Nav }
  | { kind: "activate"; row: MenuRow }
  | { kind: "close" }
  /** The bar's own arrows: the menu to the left or right opens instead. */
  | { kind: "bar"; step: -1 | 1 }
  | { kind: "none" };

const STATE = (path: number[]): NavResult => ({ kind: "state", nav: { path } });

export function rowsAt(root: readonly MenuRow[], path: readonly number[], depth: number): readonly MenuRow[] {
  let rows = root;
  for (let level = 0; level < depth; level++) rows = rows[path[level]!]?.children ?? [];
  return rows;
}

const live = (row: MenuRow | undefined) => row !== undefined && row.enabled && !row.separator;

export function firstEnabled(rows: readonly MenuRow[]): number {
  return rows.findIndex(live);
}

const opens = (row: MenuRow | undefined) => live(row) && row!.children.length > 0;

/** A key pressed while a menu is open. Escape closes the innermost submenu before the menu;
    rows that are off are stepped over and the arrows go round the ends. */
export function navKey(root: readonly MenuRow[], nav: Nav, key: string, bar = false): NavResult {
  const depth = nav.path.length - 1;
  const rows = rowsAt(root, nav.path, depth);
  const at = nav.path[depth] ?? -1;
  const row = rows[at];

  if (key === "Tab") return { kind: "close" };
  if (key === "Escape") return depth > 0 ? STATE(nav.path.slice(0, -1)) : { kind: "close" };
  if (key === "ArrowLeft") {
    if (depth > 0) return STATE(nav.path.slice(0, -1));
    return bar ? { kind: "bar", step: -1 } : { kind: "none" };
  }
  if (key === "ArrowRight" || key === "Enter" || key === " ") {
    if (opens(row)) return STATE([...nav.path, firstEnabled(row!.children)]);
    if (key === "ArrowRight") return bar && depth === 0 ? { kind: "bar", step: 1 } : { kind: "none" };
    return live(row) ? { kind: "activate", row: row! } : { kind: "none" };
  }
  const move = menuKey(key, at < 0 ? null : at, rows.map(live));
  if (move?.kind === "focus") return STATE([...nav.path.slice(0, -1), move.to]);
  return { kind: "none" };
}

/** The pointer is over row `index` of level `depth`: that row is focused, anything open
    deeper closes. */
export function hoverRow(nav: Nav, depth: number, index: number): Nav {
  return { path: [...nav.path.slice(0, depth), index] };
}

/** Opens the submenu of the focused row, as the pointer does after a short delay. */
export function openFocused(root: readonly MenuRow[], nav: Nav): Nav {
  const depth = nav.path.length - 1;
  const row = rowsAt(root, nav.path, depth)[nav.path[depth] ?? -1];
  return opens(row) ? { path: [...nav.path, -1] } : nav;
}

/** The row a typed letter jumps to: the next enabled row whose label starts with what was
    typed, going round; the focused row stays when it still matches a longer word. */
export function typeaheadTo(rows: readonly MenuRow[], at: number, typed: string): number | null {
  const word = typed.toLowerCase();
  if (word === "" || rows.length === 0) return null;
  const hits = (row: MenuRow) => live(row) && row.label.toLowerCase().startsWith(word);
  if (word.length > 1 && at >= 0 && hits(rows[at]!)) return at;
  for (let step = 1; step <= rows.length; step++) {
    const index = (Math.max(at, -1) + step + rows.length) % rows.length;
    if (hits(rows[index]!)) return index;
  }
  return null;
}

/** What has been typed in the last second, so `Pu` narrows instead of cycling. */
export class TypedWord {
  #word = "";
  #at = 0;
  constructor(private readonly holdMs = 1000) {}

  add(letter: string, now: number): string {
    this.#word = now - this.#at > this.holdMs ? letter : this.#word + letter;
    this.#at = now;
    return this.#word;
  }
}

/** The letter each top-level title answers to with Alt: the first one nobody took before it. */
export function mnemonics(titles: readonly string[]): (number | null)[] {
  const taken = new Set<string>();
  return titles.map((title) => {
    for (let index = 0; index < title.length; index++) {
      const letter = title[index]!.toLowerCase();
      if (!/[a-z0-9]/.test(letter) || taken.has(letter)) continue;
      taken.add(letter);
      return index;
    }
    return null;
  });
}
