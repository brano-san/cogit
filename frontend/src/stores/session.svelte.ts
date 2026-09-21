import {
  forget,
  readSession,
  withActive,
  withOpened,
  withSelected,
  writeSession,
  type Session,
} from "$lib/session";

const STORAGE_KEY = "cogit.session.v1";
const WRITE_DELAY_MS = 400;

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
  #pending: ReturnType<typeof setTimeout> | undefined;

  get repositories(): readonly string[] {
    return this.#session.repositories;
  }

  get active(): string | null {
    return this.#session.active;
  }

  get recent(): readonly string[] {
    return this.#session.recent;
  }

  /** Called on every open, so the start screen reflects what was really used. */
  opened(root: string): void {
    this.write(withOpened(this.#session, root));
  }

  forgetRecent(root: string): void {
    this.write({ ...this.#session, recent: forget(this.#session.recent, root) });
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
      recent: this.#session.recent,
    });
  }

  setActive(root: string | null): void {
    this.write(withActive(this.#session, root));
  }

  setSelected(root: string, oid: string | null): void {
    this.write(withSelected(this.#session, root, oid));
  }

  /** The selected commit changes on every arrow key, so the write is deferred: a
      synchronous store round trip per keystroke is felt in the list. What is already in
      memory is correct by construction; validation is for what comes off disk. */
  private write(next: Session): void {
    // A write that changed nothing must not touch the field: `$state.raw` invalidates on
    // the reference alone, and an effect that reads the session and writes it back would
    // never settle (R-87).
    if (next === this.#session) return;

    this.#session = next;
    clearTimeout(this.#pending);
    this.#pending = setTimeout(() => this.persist(), WRITE_DELAY_MS);
  }

  /** Called on the way out, so a close does not lose the last few seconds. */
  persist(): void {
    clearTimeout(this.#pending);
    try {
      localStorage.setItem(STORAGE_KEY, writeSession(this.#session));
    } catch {
      // A blocked store costs the user the layout on restart, nothing more.
    }
  }
}

export const session = new SessionStore();
