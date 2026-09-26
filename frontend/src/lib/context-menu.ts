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

/** Drops separators that separate nothing — leading, trailing and doubled ones — so a menu
    can be written as a flat list and each row can switch itself off (the same rule
    `menu::tidy` applies in Rust, R-133). Every context menu goes through it. */
export function tidy(entries: readonly ContextItem[]): ContextItem[] {
  const kept: ContextItem[] = [];
  for (const entry of entries) {
    if (entry.separator && (kept.length === 0 || kept[kept.length - 1]!.separator)) continue;
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
