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

/** A reload shorter than this never lights the dot. */
export const STALE_SHOW_MS = 600;
/** How long a lit dot waits for the next reload before it goes out. */
export const STALE_LINGER_MS = 1500;

/** The dot a panel shows while it is behind the disk (F-223). A dot for every reload
    blinked with every write to the working tree; this one lights only for a reload that
    takes a while and, once lit, outlasts the gaps between reloads (#35). */
export class StaleDot {
  #shown = false;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #pending: boolean | null = null;

  constructor(private readonly onchange: (shown: boolean) => void) {}

  set(stale: boolean): void {
    if (this.#pending === stale) return;
    this.#cancel();
    if (stale === this.#shown) return;
    this.#pending = stale;
    this.#timer = setTimeout(
      () => {
        this.#timer = null;
        this.#pending = null;
        this.#shown = stale;
        this.onchange(stale);
      },
      stale ? STALE_SHOW_MS : STALE_LINGER_MS,
    );
  }

  dispose(): void {
    this.#cancel();
  }

  #cancel(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#pending = null;
  }
}
