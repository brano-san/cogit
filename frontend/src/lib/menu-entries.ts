import type { ContextItem } from "./ipc";

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

export function submenu(id: string, label: string, children: ContextItem[], enabled = true): ContextItem {
  return { ...entry(id, label, enabled && children.length > 0), children };
}

export function dropStraySeparators(entries: readonly ContextItem[]): ContextItem[] {
  const kept: ContextItem[] = [];
  for (const each of entries) {
    if (each.separator && (kept.length === 0 || kept[kept.length - 1]!.separator)) continue;
    kept.push(each.children ? { ...each, children: dropStraySeparators(each.children) } : each);
  }
  while (kept.length > 0 && kept[kept.length - 1]!.separator) kept.pop();
  return kept;
}
