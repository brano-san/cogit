import { keyLetter } from "./key-letter";
import { ON_MAC, primary, type Modifiers } from "./platform";

/** Ctrl+A / Cmd+A, with nothing else held. */
export function isSelectAllKey(
  event: Modifiers & { key: string; code?: string; shiftKey: boolean; altKey: boolean },
  onMac: boolean,
): boolean {
  return primary(event, onMac) && !event.shiftKey && !event.altKey && keyLetter(event) === "a";
}

export interface SelectAllContext {
  /** The key landed in a text field, an editable Result or a CodeMirror editor. */
  editable: boolean;
  /** Inside something that answers Ctrl+A itself (the output window). */
  own: boolean;
  /** The panel the keyboard talks to; null in a child window, which has one pane of its own. */
  panel: string | null;
  /** The panels that registered a Select All. */
  lists: ReadonlySet<string>;
  /** The text region last touched: its panel (`data-select-text`), or null when not in one. */
  region: { panel: string | null } | null;
}

/** What Ctrl+A does. Only `native` leaves the browser's own select-all alone: it would
    otherwise select the whole document, so every other answer is `preventDefault`. */
export type SelectAllAction = "native" | "own" | "list" | "text" | "none";

export function decideSelectAll(context: SelectAllContext): SelectAllAction {
  if (context.editable) return "native";
  if (context.own) return "own";
  if (context.panel !== null && context.lists.has(context.panel)) return "list";
  const region = context.region;
  if (region !== null && (context.panel === null || region.panel === context.panel)) return "text";
  return "none";
}

const FIELD_TYPES = new Set(["text", "search", "url", "tel", "email", "password", "number", ""]);

/** A field that has its own select-all. Checkboxes and buttons are not: Ctrl+A there is
    the list's. */
export function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target.closest(".cm-editor") !== null) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  return target instanceof HTMLInputElement && FIELD_TYPES.has(target.type);
}

export const TEXT_REGION = "data-select-text";
export const OWN_SELECTION = "data-select-own";

/** The text region (a diff side, the commit details) holding `node`. */
export function regionOf(node: Node | null): HTMLElement | null {
  const element = node instanceof Element ? node : (node?.parentElement ?? null);
  return element?.closest<HTMLElement>(`[${TEXT_REGION}]`) ?? null;
}

export function selectContents(region: HTMLElement): void {
  const selection = region.ownerDocument.defaultView?.getSelection();
  if (!selection) return;
  const range = region.ownerDocument.createRange();
  range.selectNodeContents(region);
  selection.removeAllRanges();
  selection.addRange(range);
}

const lists = new Map<string, () => void>();

/** A panel with several rows hands its Select All over; the last to register wins. Returns
    the undo. */
export function registerSelectAll(panel: string, run: () => void): () => void {
  lists.set(panel, run);
  return () => {
    if (lists.get(panel) === run) lists.delete(panel);
  };
}

let fromMenu: (() => void) | null = null;

/** Edit ▸ Select All of the native menu: the same answer as the key, for the element the
    focus is in. The menu item carries no accelerator, so the key itself reaches the page. */
export function selectAllFromMenu(): void {
  fromMenu?.();
}

/** The one Ctrl+A handler of a window. `active` is the panel in focus, the one whose header
    is underlined; null in a child window. */
export function installSelectAll(win: Window, active: () => string | null): () => void {
  let touched: HTMLElement | null = null;

  /** `key`: the event of the key press, absent for the menu item. */
  function answer(target: EventTarget | null, key?: KeyboardEvent) {
    const region = touched?.isConnected === true ? touched : regionOf(win.document.activeElement);
    const panel = active();
    const action = decideSelectAll({
      editable: isEditable(target),
      own: target instanceof Element && target.closest(`[${OWN_SELECTION}]`) !== null,
      panel,
      lists: new Set(lists.keys()),
      region: region === null ? null : { panel: region.getAttribute(TEXT_REGION) || null },
    });
    if (action === "native") {
      // A key press selects the field's text by itself; the menu has to ask for it.
      if (!key) win.document.execCommand("selectAll");
      return;
    }
    if (action === "own") return;
    key?.preventDefault();
    if (action === "list" && panel !== null) lists.get(panel)?.();
    else if (action === "text" && region !== null) selectContents(region);
  }

  const pointerdown = (event: PointerEvent) => {
    touched = regionOf(event.target as Node | null);
  };
  const keydown = (event: KeyboardEvent) => {
    if (event.defaultPrevented || !isSelectAllKey(event, ON_MAC)) return;
    answer(event.target, event);
  };
  const menu = () => answer(win.document.activeElement);
  win.addEventListener("pointerdown", pointerdown, true);
  win.addEventListener("keydown", keydown);
  fromMenu = menu;
  return () => {
    win.removeEventListener("pointerdown", pointerdown, true);
    win.removeEventListener("keydown", keydown);
    if (fromMenu === menu) fromMenu = null;
  };
}
