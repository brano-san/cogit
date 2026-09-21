import { readSession, writeSession, type Session } from "$lib/session";

const STORAGE_KEY = "cogit.session.v1";

function stored(): Session {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return readSession(raw === null ? null : (JSON.parse(raw) as unknown));
  } catch {
    return readSession(null);
  }
}

class SessionStore {
  #session = $state.raw<Session>(stored());

  get repositories(): readonly string[] {
    return this.#session.repositories;
  }

  get active(): string | null {
    return this.#session.active;
  }

  selected(root: string): string | null {
    return this.#session.selected[root] ?? null;
  }

  /** The open list, after every change to it. Anything that left takes its record along. */
  remember(roots: readonly string[]): void {
    const selected: Record<string, string> = {};
    for (const root of roots) {
      const oid = this.#session.selected[root];
      if (oid) selected[root] = oid;
    }
    this.write({
      repositories: [...roots],
      active: this.#session.active !== null && roots.includes(this.#session.active)
        ? this.#session.active
        : (roots[0] ?? null),
      selected,
    });
  }

  setActive(root: string | null): void {
    this.write({ ...this.#session, active: root });
  }

  setSelected(root: string, oid: string | null): void {
    const selected = { ...this.#session.selected };
    if (oid === null) delete selected[root];
    else selected[root] = oid;
    this.write({ ...this.#session, selected });
  }

  private write(next: Session): void {
    this.#session = readSession(next);
    try {
      localStorage.setItem(STORAGE_KEY, writeSession(this.#session));
    } catch {
      // A blocked store costs the user the layout on restart, nothing more.
    }
  }
}

export const session = new SessionStore();
