import { ACTIONS, DEFAULT_LAYOUT, SEPARATOR, type ToolbarAction } from "$lib/toolbar";

/** Unknown ids and repeats dropped, separators never doubled or at an end; not a list: default. */
export function normalizeLayout(stored: unknown): string[] {
  if (!Array.isArray(stored)) return [...DEFAULT_LAYOUT];
  const known = new Set(ACTIONS.map((action) => action.id));
  const seen = new Set<string>();
  const out: string[] = [];
  for (const entry of stored) {
    if (entry === SEPARATOR) {
      if (out.length > 0 && out[out.length - 1] !== SEPARATOR) out.push(SEPARATOR);
      continue;
    }
    if (typeof entry !== "string" || !known.has(entry) || seen.has(entry)) continue;
    seen.add(entry);
    out.push(entry);
  }
  while (out[out.length - 1] === SEPARATOR) out.pop();
  return out;
}

export function hiddenActions(layout: readonly string[]): ToolbarAction[] {
  return ACTIONS.filter((action) => !layout.includes(action.id));
}

/** Inserts after `after`, or at the end when nothing is selected. */
export function addEntry(layout: readonly string[], entry: string, after: number | null): string[] {
  const at = after === null ? layout.length : Math.min(after + 1, layout.length);
  return [...layout.slice(0, at), entry, ...layout.slice(at)];
}

export function removeEntry(layout: readonly string[], index: number): string[] {
  return layout.filter((_, at) => at !== index);
}

export function moveEntry(layout: readonly string[], index: number, delta: -1 | 1): string[] {
  const target = index + delta;
  if (index < 0 || index >= layout.length || target < 0 || target >= layout.length) {
    return [...layout];
  }
  const next = [...layout];
  [next[index], next[target]] = [next[target] as string, next[index] as string];
  return next;
}

export function sameLayout(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((entry, at) => entry === b[at]);
}
