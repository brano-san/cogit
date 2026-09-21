export interface Session {
  repositories: string[];
  /** The one that was in front; null when the list is empty or the record is stale. */
  active: string | null;
  /** Repository path to the commit that was selected in it. */
  selected: Record<string, string>;
  /** Everything opened lately, newest first. Kept after a repository is closed: that is
      the whole point of the start screen. */
  recent: string[];
}

const EMPTY: Session = { repositories: [], active: null, selected: {}, recent: [] };

const RECENT_LIMIT = 20;

export function remember(recent: readonly string[], root: string): string[] {
  return [root, ...recent.filter((path) => path !== root)].slice(0, RECENT_LIMIT);
}

export function forget(recent: readonly string[], root: string): string[] {
  return recent.filter((path) => path !== root);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Anything unrecognised is dropped: a stale record must cost at most a closed tab. */
export function readSession(raw: unknown): Session {
  if (!isRecord(raw)) return { ...EMPTY, selected: {}, recent: [] };

  const repositories = Array.isArray(raw.repositories)
    ? [...new Set(raw.repositories.filter((path): path is string => typeof path === "string"))]
    : [];

  const active =
    typeof raw.active === "string" && repositories.includes(raw.active) ? raw.active : null;

  const selected: Record<string, string> = {};
  if (isRecord(raw.selected)) {
    for (const [path, oid] of Object.entries(raw.selected)) {
      if (typeof oid === "string" && repositories.includes(path)) selected[path] = oid;
    }
  }

  const recent = Array.isArray(raw.recent)
    ? [...new Set(raw.recent.filter((path): path is string => typeof path === "string"))].slice(
        0,
        RECENT_LIMIT,
      )
    : [];

  return { repositories, active, selected, recent };
}

export function writeSession(session: Session): string {
  return JSON.stringify(session);
}
