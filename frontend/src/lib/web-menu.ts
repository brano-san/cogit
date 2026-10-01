import type { ContextItem, MenuNode } from "$lib/ipc/bindings";
import { tidy } from "./context-menu";
import { prettyKeys } from "./keymap";
import type { MenuRow } from "./menu-nav";

const shortcut = (keys: string | null | undefined, onMac: boolean) =>
  keys ? prettyKeys(keys, onMac) : null;

/** A context menu as the page draws it: separators tidied at every depth (R-133), keys
    formatted for the platform. */
export function contextRows(items: readonly ContextItem[], onMac: boolean): MenuRow[] {
  const rows = (entries: readonly ContextItem[]): MenuRow[] =>
    entries.map((entry) => ({
      id: entry.id,
      label: entry.label,
      enabled: entry.enabled,
      separator: entry.separator ?? false,
      shortcut: shortcut(entry.accelerator, onMac),
      checked: null,
      children: rows(entry.children ?? []),
    }));
  return rows(tidy(items));
}

/** What the application last pushed to the bar, by id. */
export interface BarState {
  disabled: ReadonlySet<string>;
  checked: ReadonlySet<string>;
  maximized: boolean;
}

/** The bar's menus as the page draws them. Live state wins over the model's own flags
    once the application has pushed any; `Window ▸ Maximize` reads `Restore` while maximized. */
export function barRows(nodes: readonly MenuNode[], onMac: boolean, state: BarState | null): MenuRow[] {
  const rows = (entries: readonly MenuNode[]): MenuRow[] =>
    tidy(entries).map((node) => ({
      id: node.id,
      label: node.id === "window-maximize" && state?.maximized ? "Restore" : node.label,
      enabled: state && node.children.length === 0 ? !state.disabled.has(node.id) : node.enabled,
      separator: node.separator,
      shortcut: shortcut(node.accelerator, onMac),
      checked: node.checked === null ? null : state ? state.checked.has(node.id) : node.checked,
      children: rows(node.children),
    }));
  return rows(nodes);
}

/** Alt+letter and F10 only answer when nothing else is meant by the key. */
export function opensBar(event: { key: string; altKey: boolean; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): boolean {
  if (event.key === "F10") return !event.ctrlKey && !event.altKey && !event.metaKey;
  return event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey && /^[a-z0-9]$/i.test(event.key);
}
