import type { ChangeKind } from "$lib/ipc";
import type { PanelId } from "$lib/perspectives";

/** Which panels a change on disk makes out of date, until their reload lands. */
const AFFECTS: Record<ChangeKind, PanelId[]> = {
  head: ["graph", "refs", "files", "commit", "diff"],
  refs: ["refs", "graph"],
  index: ["files", "commit", "diff"],
  workingTree: ["files", "diff", "repositories"],
  stash: ["refs"],
  config: ["repositories"],
  hooks: [],
  mailmap: ["graph", "commit"],
};

export function affected(kind: ChangeKind): PanelId[] {
  return AFFECTS[kind] ?? [];
}

export function mark(stale: ReadonlySet<PanelId>, kind: ChangeKind): Set<PanelId> {
  const next = new Set(stale);
  for (const panel of affected(kind)) next.add(panel);
  return next;
}

/** `since`: changes that arrived after the reload began; the panels they touch stay stale
    until a reload that began after them. */
export function clear(
  stale: ReadonlySet<PanelId>,
  panels: readonly PanelId[],
  since: Iterable<ChangeKind> = [],
): Set<PanelId> {
  const kept = new Set([...since].flatMap(affected));
  const next = new Set(stale);
  for (const panel of panels) if (!kept.has(panel)) next.delete(panel);
  return next;
}
