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
};

export function affected(kind: ChangeKind): PanelId[] {
  return AFFECTS[kind] ?? [];
}

export function mark(stale: ReadonlySet<PanelId>, kind: ChangeKind): Set<PanelId> {
  const next = new Set(stale);
  for (const panel of affected(kind)) next.add(panel);
  return next;
}

export function clear(stale: ReadonlySet<PanelId>, panels: readonly PanelId[]): Set<PanelId> {
  const next = new Set(stale);
  for (const panel of panels) next.delete(panel);
  return next;
}
