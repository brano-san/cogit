import { lostCommits, type CommitRow, type RepoId } from "$lib/ipc";

interface Flight {
  repo: RepoId;
  /** Asked for again while reading: one more read follows, not one per ask. */
  again: boolean;
  done: Promise<void>;
}

class RecoveryStore {
  lost = $state.raw<CommitRow[]>([]);
  /** Only the newest read writes; `clear()` drops the ones in flight, which belong to the
      repository the panels are leaving. */
  #generation = 0;
  /** One read per repository at a time: the first scan of its objects can take seconds on
      a cold disk, and every refresh meanwhile would start that scan over (R-731). */
  #flight: Flight | null = null;

  refresh(repo: RepoId): Promise<void> {
    const running = this.#flight;
    if (running && running.repo === repo) {
      running.again = true;
      return running.done;
    }
    const flight: Flight = { repo, again: false, done: Promise.resolve() };
    this.#flight = flight;
    flight.done = this.#readWhileAsked(flight).finally(() => {
      if (this.#flight === flight) this.#flight = null;
    });
    return flight.done;
  }

  async #readWhileAsked(flight: Flight): Promise<void> {
    do {
      flight.again = false;
      await this.#read(flight.repo);
    } while (flight.again && this.#flight === flight);
  }

  async #read(repo: RepoId): Promise<void> {
    const generation = ++this.#generation;
    let rows: CommitRow[] = [];
    await lostCommits(repo, (chunk) => {
      if (generation !== this.#generation) return;
      rows = [...rows, ...chunk];
      this.lost = rows;
    });
    // An empty answer sends no chunk, so the old list would stay.
    if (generation === this.#generation) this.lost = rows;
  }

  clear(): void {
    this.#generation += 1;
    this.#flight = null;
    this.lost = [];
  }
}

export const recovery = new RecoveryStore();
