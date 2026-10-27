import type { ContextItem } from "./ipc";

export const SEPARATOR: ContextItem = { id: "", label: "", enabled: false, separator: true };

export function item(id: string, label: string, enabled = true, accelerator?: string): ContextItem {
  return { id, label, enabled, separator: false, accelerator: accelerator ?? null };
}

/** A row that is off says why in its own label: a native menu has no tooltip for a
    disabled item (doc/12-risks.md, R-250). `null` means the row is on. */
export function offer(
  id: string,
  label: string,
  blocked: string | null,
  accelerator?: string,
): ContextItem {
  return blocked === null
    ? item(id, label, true, accelerator)
    : item(id, `${label} (${blocked})`, false, accelerator);
}

/** What `tidy` reads of a row; a `ContextItem` and the menu bar model both are one. */
interface Tidyable {
  separator?: boolean;
  enabled: boolean;
  children?: readonly unknown[];
}

/** Drops separators that separate nothing — leading, trailing and doubled ones — at every
    depth, so a menu can be written as a flat list and each row can switch itself off (the
    same rule `menu::tidy` and `tidy_items` apply in Rust, R-133). A submenu left with
    nothing inside stays as a row that is off. Every context menu goes through it. */
export function tidy<T extends Tidyable>(entries: readonly T[]): T[] {
  const kept: T[] = [];
  for (const entry of entries) {
    if (entry.separator && (kept.length === 0 || kept[kept.length - 1]!.separator)) continue;
    if (entry.children && entry.children.length > 0) {
      const inside = tidy(entry.children as Tidyable[]);
      kept.push({ ...entry, children: inside, enabled: entry.enabled && inside.length > 0 });
      continue;
    }
    kept.push(entry);
  }
  while (kept.length > 0 && kept[kept.length - 1]!.separator) kept.pop();
  return kept;
}

/** A row that opens a nested menu (`Move To ▸`); its children are tidied like a menu. */
export function submenu(id: string, label: string, children: ContextItem[], enabled = true): ContextItem {
  const inside = tidy(children);
  return { ...item(id, label, enabled && inside.length > 0), children: inside };
}

export function refMenu(at: { kind: string }): ContextItem[] {
  if (at.kind !== "lost") return [];
  return [
    item("restore-lost", "Create a branch here"),
    SEPARATOR,
    item("lost-copy-sha", "Copy the full SHA"),
    SEPARATOR,
    item("lost-toggle", "Toggle"),
  ];
}

const COMMAND_OF: Record<string, string> = {
  "file-stage": "stage",
  "file-unstage": "unstage",
  "file-stash": "stash-selection",
  "file-blame": "blame",
  "file-commit": "commit",
  "ref:wt-commit": "commit",
  "ref:wt-stage": "stage",
  "ref:wt-unstage": "unstage",
  "ref:add-branch": "branch",
  "ref:add-tag": "tag",
  "ref:copy-id": "copy-sha",
  "ref:rebase-i": "rebase-i",
  "fetch-remotes": "fetch-all",
};

const FIXED_KEYS: Record<string, string> = {
  "file-discard": "CmdOrCtrl+Z",
  "file-investigate": "CmdOrCtrl+Alt+Shift+L",
  "ref:toggle": "Space",
  "refgroup:toggle": "Space",
};

export function withKeys(items: readonly ContextItem[], keys: Readonly<Record<string, string>>): ContextItem[] {
  return items.map((entry) => {
    const command = COMMAND_OF[entry.id];
    const accelerator =
      entry.accelerator || (command !== undefined ? keys[command] : undefined) || FIXED_KEYS[entry.id] || null;
    const children = entry.children ? withKeys(entry.children, keys) : entry.children;
    return { ...entry, accelerator, ...(children ? { children } : {}) };
  });
}

let menuKeys: Readonly<Record<string, string>> = {};

export function setMenuKeys(keys: Readonly<Record<string, string>>): void {
  menuKeys = keys;
}

export function withMenuKeys(items: readonly ContextItem[]): ContextItem[] {
  return withKeys(items, menuKeys);
}
