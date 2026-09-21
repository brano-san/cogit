export interface Session {
  repositories: string[];
  /** The one that was in front; null when the list is empty or the record is stale. */
  active: string | null;
  /** Repository path to the commit that was selected in it. */
  selected: Record<string, string>;
}

const EMPTY: Session = { repositories: [], active: null, selected: {} };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Anything unrecognised is dropped: a stale record must cost at most a closed tab. */
export function readSession(raw: unknown): Session {
  if (!isRecord(raw)) return { ...EMPTY, selected: {} };

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

  return { repositories, active, selected };
}

export function writeSession(session: Session): string {
  return JSON.stringify(session);
}
