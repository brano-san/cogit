import { headLabel, splitBranches } from "$lib/format";
import {
  closeRepository,
  CogitError,
  listRepositories,
  openRepository,
  repoRefs,
  rereadRepository,
  type RepoId,
  type RepoOverview,
  type RepoSummary,
  toCogitError,
  workingState,
} from "$lib/ipc";
import { trace } from "$lib/trace";
import { notices } from "$stores/notices.svelte";
import { repoList } from "$stores/repo-list.svelte";
import { session } from "$stores/session.svelte";

/** Opening a repository is one transition, and every panel reads the result of it from
    here rather than catching an event as it goes past. A panel mounted late sees the
    same thing as one mounted early, which is what the Graph panel did not (R-98).

    `repo` rides along on `opening` and `failed` so that re-reading an open repository —
    which the file watcher does on every ref move — cannot blank the panels while it is
    in flight or if it goes wrong. */
export type RepoPhase =
  | { kind: "closed" }
  | { kind: "opening"; root: string; repo: RepoSummary | null }
  | { kind: "open"; repo: RepoSummary }
  | { kind: "failed"; root: string; error: CogitError; repo: RepoSummary | null };

/** Long enough that a cold repository with submodules is not called a failure, short
    enough that nobody watches a spinner wondering whether it is still alive. */
const OPEN_TIMEOUT_MS = 30_000;

function asCogitError(err: unknown): CogitError {
  return toCogitError(err);
}

class RepositoryStore {
  phase = $state.raw<RepoPhase>({ kind: "closed" });
  openRepos = $state.raw<RepoOverview[]>([]);

  readonly openTimeoutMs = OPEN_TIMEOUT_MS;

  /** Only the newest open may write the phase; an older one that finishes late is
      dropped, whichever way it ends. */
  #ticket = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;
  /** What a listed repository looked like when a submodule or worktree of it replaced it
      in the panels, so clicking it again switches back without reopening it (#50). */
  #kept = new Map<string, RepoSummary>();
  #epoch = 0;
  /** A change arrived while a re-read was on its way; that re-read may predate it. */
  #again = false;

  /** Changes whenever the panels are about to show another repository, never on a
      re-read of the same one. Work begun under an older value belongs to a repository
      the user has left and must not write its answer. */
  get epoch(): number {
    return this.#epoch;
  }

  #leaves = new Set<() => void>();

  /** Called, synchronously, whenever `epoch` changes. Returns the unsubscribe. */
  onLeave(listener: () => void): () => void {
    this.#leaves.add(listener);
    return () => this.#leaves.delete(listener);
  }

  #leaving(root: string): void {
    if (this.current?.root !== root) this.#left();
  }

  #left(): void {
    this.#epoch += 1;
    for (const listener of [...this.#leaves]) listener();
  }

  get current(): RepoSummary | null {
    return this.phase.kind === "closed" ? null : this.phase.repo;
  }

  get busy(): boolean {
    return this.phase.kind === "opening";
  }

  get error(): CogitError | null {
    return this.phase.kind === "failed" ? this.phase.error : null;
  }

  get localBranches() {
    return splitBranches(this.current?.branches ?? []).local;
  }

  get remoteBranches() {
    return splitBranches(this.current?.branches ?? []).remote;
  }

  get headLabel(): string {
    return headLabel(this.current?.head);
  }

  /** `false` when a newer open overtook this one and its answer was dropped. */
  open(path: string): Promise<boolean> {
    return this.#read(path, () => openRepository(path));
  }

  /** An open of `path`, whichever command reads it: the phase goes through `opening`. */
  async #read(path: string, read: () => Promise<RepoSummary>): Promise<boolean> {
    const ticket = this.#begin(path);
    try {
      const repo = await read();
      trace(`open:${path}`, `backend answered, ticket ${ticket}, ${repo.branches.length} refs`);
      return this.#settle(ticket, { kind: "open", repo });
    } catch (err) {
      trace(`open:${path}`, `backend refused, ticket ${ticket}: ${String(err)}`);
      return this.#settle(ticket, {
        kind: "failed",
        root: path,
        error: asCogitError(err),
        repo: this.current,
      });
    }
  }

  #begin(root: string): number {
    const ticket = ++this.#ticket;
    this.#leaving(root);
    this.#disarm();
    this.phase = { kind: "opening", root, repo: this.current };
    trace(`open:${root}`, `phase → opening, ticket ${ticket}`);
    // A backend that never answers is a bug of its own, but it must not read as
    // "still working" for ever. A real answer arriving later still wins.
    this.#timer = setTimeout(() => {
      if (this.#ticket !== ticket || this.phase.kind !== "opening") return;
      trace(`open:${root}`, `phase → failed, timed out after ${OPEN_TIMEOUT_MS}ms`);
      this.phase = {
        kind: "failed",
        root,
        error: new CogitError({
          kind: "internal",
          data: `Opening ${root} is taking longer than ${Math.round(OPEN_TIMEOUT_MS / 1000)}s. It may still be running; the log says how far it got.`,
        }),
        repo: this.current,
      };
    }, OPEN_TIMEOUT_MS);
    return ticket;
  }

  #settle(ticket: number, phase: RepoPhase): boolean {
    if (this.#ticket !== ticket) {
      trace("open", `ticket ${ticket} is stale, ${this.#ticket} is current; answer dropped`);
      return false;
    }
    this.#disarm();
    this.phase = phase;
    trace("open", `phase → ${phase.kind}, ticket ${ticket}`);
    return true;
  }

  #disarm(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }

  #statusRead = 0;

  /** Staging changes only the counters; re-reading every ref for that is waste (R-24).
      The same read lists the conflicted paths, returned for the conflicts store; `null`
      when the answer was dropped or never came (R-316). */
  async refreshStatus(): Promise<string[] | null> {
    const repo = this.current?.repo;
    if (!repo) return null;
    const ticket = this.#ticket;
    const asked = ++this.#statusRead;
    try {
      const { status, conflicted, indexLock } = await workingState(repo);
      // Reads run side by side: an older answer arriving last holds older counters.
      if (this.#ticket !== ticket || asked !== this.#statusRead) return null;
      const open = this.current;
      if (!open || open.repo !== repo) return null;
      this.#replace({ ...open, status, indexLock });
      return conflicted;
    } catch {
      // Nothing actionable; the next full refresh reports it with its own error.
      return null;
    }
  }

  /** Takes a repository somebody else opened — a submodule reached from the tree —
      without asking the backend to open it again. */
  adopt(repo: RepoSummary): void {
    this.#ticket += 1;
    this.#leaving(repo.root);
    this.#disarm();
    this.phase = { kind: "open", repo };
  }

  /** Called before a submodule or worktree takes the panels. `repo` is for an owner that
      was opened without ever being shown: a submodule reached from its light tree. */
  keep(repo: RepoSummary | null = this.current): void {
    if (repo) this.#kept.set(repo.root, repo);
  }

  /** Shows the kept summary at once and re-reads it quietly, with no `opening` phase: a
      worktree may have moved a branch meanwhile. Nothing kept is an ordinary open. */
  async comeBack(root: string): Promise<void> {
    const kept = this.#kept.get(root);
    this.#kept.delete(root);
    if (!kept) {
      await this.open(root);
      return;
    }
    this.adopt(kept);
    const ticket = this.#ticket;
    try {
      const fresh = await openRepository(root);
      if (this.#ticket === ticket) this.#replace(fresh);
    } catch (err) {
      trace(`open:${root}`, `re-reading the kept repository failed: ${String(err)}`);
    }
  }

  /** The repository is the same one; only its contents were re-read. */
  #replace(repo: RepoSummary): void {
    if (this.phase.kind === "closed") return;
    this.phase = this.phase.kind === "open" ? { kind: "open", repo } : { ...this.phase, repo };
  }

  /** An open in flight already brings fresh contents, and `current` during it is the
      repository being left: re-reading that one would cancel the open. */
  async refresh(): Promise<void> {
    if (this.phase.kind === "opening") {
      // A re-read of the repository on screen, begun before this change: one more after it.
      if (this.phase.repo?.root === this.phase.root) this.#again = true;
      return;
    }
    const current = this.current;
    if (!current) return;
    const { root, repo } = current;
    do {
      this.#again = false;
      // By id: opening the folder again would list a submodule or worktree (R-543).
      const shown = await this.#read(root, () => rereadRepository(repo));
      if (!shown || this.current?.root !== root) return;
    } while (this.#again);
  }

  #refsRead = 0;

  /** After a commit: the refs and the state are what moved besides the counters, which
      `refreshStatus` reads. Reopening read the status as well, went through `opening`,
      and re-registered the repository (R-316). Anything but a settled open takes the
      ordinary `refresh`, which knows how to wait for an open in flight. */
  async refreshRefs(): Promise<void> {
    if (this.phase.kind !== "open") return this.refresh();
    const repo = this.phase.repo.repo;
    const ticket = this.#ticket;
    const asked = ++this.#refsRead;
    try {
      const refs = await repoRefs(repo);
      // An open begun since brings newer contents; a later re-read of the refs, newer refs.
      if (this.#ticket !== ticket || asked !== this.#refsRead) return;
      const open = this.current;
      if (open && open.repo === repo) this.#replace({ ...open, ...refs });
    } catch (err) {
      trace("refs", `re-reading the refs failed, reopening: ${String(err)}`);
      if (this.#ticket === ticket) await this.refresh();
    }
  }

  /** Closing one repository while another opens asks twice; the older answer must not
      bring back what was closed, nor drop what was opened, from the next session too. */
  #listed = 0;

  async refreshList(): Promise<void> {
    const asked = ++this.#listed;
    this.#takeList(asked, await listRepositories());
  }

  #takeList(asked: number, list: RepoOverview[]): void {
    if (asked !== this.#listed) return;
    this.openRepos = list;
    session.remember(this.openRepos.map((entry) => entry.root));
  }

  /** Reopens everything the previous session had. One that does not open — its folder
      moved, its drive not mounted yet — stays in the list as a closed row, which its pulse
      marks missing, and is named in a notice: dropped, it was gone for good (T3.7). */
  async restore(): Promise<string[]> {
    const failed: string[] = [];
    const reasons: string[] = [];
    for (const root of session.repositories) {
      try {
        await openRepository(root);
      } catch (err) {
        failed.push(root);
        reasons.push(`${root}: ${asCogitError(err).message}`);
      }
    }
    for (const root of failed) repoList.closed(root);
    await this.refreshList();
    if (failed.length > 0) {
      const title =
        failed.length === 1 ? "A repository did not reopen" : `${failed.length} repositories did not reopen`;
      notices.inform(title, ["They stay in the list, closed.", ...reasons].join("\n"));
    }
    return failed;
  }

  async closeOne(repo: RepoId): Promise<void> {
    trace("close", `asked to close repository ${repo}`);
    if (this.current?.repo === repo) this.close();
    this.openRepos = this.openRepos.filter((entry) => entry.repo !== repo);
    // The close answers with the list that is left: a second call for it was one more
    // round trip, and one more frame, after the panels had already settled.
    const asked = ++this.#listed;
    try {
      const left = await closeRepository(repo);
      trace("close", `backend released repository ${repo}, ${left.length} left open`);
      this.#takeList(asked, left);
    } catch (err) {
      trace("close", `backend refused to close ${repo}: ${String(err)}`);
      await this.refreshList();
    }
  }

  close(): void {
    this.#ticket += 1;
    this.#left();
    this.#disarm();
    this.phase = { kind: "closed" };
  }
}

export const repository = new RepositoryStore();
