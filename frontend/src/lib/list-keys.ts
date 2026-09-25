import { nextRow } from "$lib/graph-geometry";

/** The keyboard of every list panel (11 §10), the graph's `nextRow` included. */
export type ListKey =
  | { kind: "move"; to: number; extend: boolean }
  | { kind: "fold"; open: boolean }
  | { kind: "activate" };

export interface ListPress {
  key: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
}

export function pressOf(event: KeyboardEvent): ListPress {
  return { key: event.key, ctrl: event.ctrlKey || event.metaKey, shift: event.shiftKey, alt: event.altKey };
}

export function listKey(press: ListPress, at: number | null, count: number, page: number): ListKey | null {
  if (press.ctrl || press.alt) return null;
  if (press.key === "ArrowLeft" || press.key === "ArrowRight") {
    return { kind: "fold", open: press.key === "ArrowRight" };
  }
  if (press.key === "Enter") return at === null ? null : { kind: "activate" };
  const to = nextRow(at, press.key, count, page);
  return to === null ? null : { kind: "move", to, extend: press.shift };
}

/** A key that types into the list's search: one printable character. Space is left to the
    row (Files ticks with it). */
export function typedChar(press: ListPress): string | null {
  if (press.ctrl || press.alt || press.key.length !== 1 || press.key === " ") return null;
  return press.key;
}

/** Letters typed in a run make one prefix; a pause starts a new one. */
export class TypeAhead {
  #text = "";
  #last = -Infinity;

  constructor(private readonly pause = 800) {}

  type(char: string, now: number): string {
    this.#text = now - this.#last > this.pause ? char : this.#text + char;
    this.#last = now;
    return this.#text;
  }
}

/** The row a typed prefix lands on. A longer prefix keeps the current row while it still
    matches; a single letter moves on to the next match, round the end. */
export function findTyped(labels: readonly string[], at: number | null, text: string): number | null {
  const wanted = text.toLowerCase();
  const count = labels.length;
  const first = at === null ? 0 : at + (text.length === 1 ? 1 : 0);
  for (let step = 0; step < count; step++) {
    const index = (((first + step) % count) + count) % count;
    if (labels[index]?.toLowerCase().startsWith(wanted)) return index;
  }
  return null;
}

/** Rows that fit in `element` less one, so a page keeps one row of context. */
export function pageRows(element: Element | null | undefined, rowHeight: number): number {
  const height = element?.clientHeight ?? 0;
  return Math.max(Math.floor(height / Math.max(rowHeight, 1)) - 1, 1);
}

/** For a list whose rows are focusable elements marked `data-key-row`, their text in
    `data-key-label`: the arrows and typing move the focus, each row keeps its own Enter.
    Returns the row it moved to. */
export function moveFocus(container: HTMLElement, event: KeyboardEvent, typing: TypeAhead): HTMLElement | null {
  const target = event.target;
  if (target instanceof HTMLTextAreaElement || (target instanceof HTMLInputElement && target.type !== "checkbox")) {
    return null;
  }
  const rows = [...container.querySelectorAll<HTMLElement>("[data-key-row]")];
  const found = rows.findIndex((row) => row.contains(document.activeElement));
  const at = found === -1 ? null : found;
  const press = pressOf(event);
  const char = typedChar(press);
  let to: number | null = null;
  if (char !== null) {
    const labels = rows.map((row) => row.dataset.keyLabel ?? row.textContent ?? "");
    to = findTyped(labels, at, typing.type(char, event.timeStamp));
  } else {
    const action = listKey(press, at, rows.length, pageRows(container, rows[0]?.offsetHeight ?? 22));
    if (action?.kind === "move") to = action.to;
  }
  const row = to === null ? undefined : rows[to];
  if (!row) return null;
  event.preventDefault();
  row.focus();
  return row;
}
