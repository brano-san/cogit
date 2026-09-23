import type { ContextItem } from "./ipc";

/** Rows for the native context menus of the repository and file lists (#36, #40, #41). */

export const SEPARATOR: ContextItem = { id: "", label: "", enabled: false, separator: true };

export function entry(id: string, label: string, enabled = true, accelerator?: string): ContextItem {
  return { id, label, enabled, separator: false, accelerator: accelerator ?? null };
}

/** A native menu shows no tooltip on a disabled row, so the reason rides in the label. */
export function explained(
  id: string,
  label: string,
  reason: string | null,
  accelerator?: string,
): ContextItem {
  return entry(id, reason === null ? label : `${label} (${reason})`, reason === null, accelerator);
}

/** A row that opens a nested menu; the popup draws the arrow itself. */
export function submenu(id: string, label: string, children: ContextItem[], enabled = true): ContextItem {
  return { ...entry(id, label, enabled && children.length > 0), children };
}

/** Same rule as `tidy` in context-menu.ts (owned by the graph menus), applied at every
    depth; to be merged into that one helper. */
export function dropStraySeparators(entries: readonly ContextItem[]): ContextItem[] {
  const kept: ContextItem[] = [];
  for (const each of entries) {
    if (each.separator && (kept.length === 0 || kept[kept.length - 1]!.separator)) continue;
    kept.push(each.children ? { ...each, children: dropStraySeparators(each.children) } : each);
  }
  while (kept.length > 0 && kept[kept.length - 1]!.separator) kept.pop();
  return kept;
}
