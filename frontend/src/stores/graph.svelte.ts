import {
  type CogitError,
  EMPTY_QUERY,
  graphRowOf,
  graphWindow,
  loadGraph,
  type CommitQuery,
  type RepoId,
  toCogitError,
} from "$lib/ipc";
import type { GraphBlock, GraphEntry } from "$lib/graph-wire";
import { repository } from "$stores/repository.svelte";

export type { GraphEntry };

/** Rows per request. A screen is about forty; the next block is asked for early. */
const BLOCK = 128;
const AHEAD = 64;
/** Blocks asked for past that, the way the list is moving, so it finds them there. */
const LEAD = 4;
/** Blocks kept per graph; the ones farthest from the screen go first. */
const KEEP = 64;

/** One walk of the history. Its rows stay in Rust and come over by block (R-193). */
interface Walk {
  repo: RepoId;
  /** Rust's number for this walk, known from its first progress message. */
  generation: number | null;
  total: number;
  complete: boolean;
  blocks: Map<number, GraphBlock>;
  /** Blocks on their way, so a second caller waits for the same answer. */
  asking: Map<number, Promise<void>>;
}

const walk = (repo: RepoId): Walk => ({
  repo,
  generation: null,
  total: 0,
  complete: false,
  blocks: new Map(),
  asking: new Map(),
});

const asError = (err: unknown) =>
  toCogitError(err);

class GraphStore {
  /** Rows laid out so far: the list is this long while the rest is walked. */
  total = $state(0);
  loading = $state(false);
  complete = $state(false);
  error = $state<CogitError | null>(null);
  skipped = $state.raw<import("$lib/ipc").SkippedRef[]>([]);

  query = $state.raw<CommitQuery>(EMPTY_QUERY);

  /** `null` is every ref. Owned by the References panel, folded into every load. */
  visibleRefs = $state.raw<string[] | null>(null);

  /** Asking twice for the same commit has to scroll twice, hence the counter. */
  reveal = $state.raw<{ oid: string; request: number } | null>(null);

  /** Bumped when rows arrive, so that whatever read `rowAt` reads again. */
  #arrived = $state(0);

  /** Bumped when another repository's history takes the list: it starts at its top. */
  home = $state(0);

  #shown: Walk | null = null;
  /** A reload catching up; the old rows stay on screen until it covers them (R-186). */
  #next: Walk | null = null;
  #range = { start: 0, end: 0 };
  /** Which way the list last moved: 1 down, -1 up, 0 not since this history came on. */
  #heading: -1 | 0 | 1 = 0;
  #loads = 0;

  constructor() {
    // A reload of the repository being left must not take the screen after it (R-300).
    repository.onLeave(() => {
      if (!this.#next) return;
      this.#loads += 1;
      this.#next = null;
      this.loading = false;
    });
  }

  /** The repository whose rows are on screen; it lags the one open while its graph loads. */
  get shownRepo(): RepoId | null {
    return this.#shown?.repo ?? null;
  }

  requestReveal(oid: string): void {
    this.reveal = { oid, request: (this.reveal?.request ?? 0) + 1 };
  }

  rowAt(index: number): GraphEntry | undefined {
    void this.#arrived;
    const block = this.#shown?.blocks.get(Math.floor(index / BLOCK));
    const at = index % BLOCK;
    return block && at < block.length ? block.entry(at) : undefined;
  }

  /** How many rows are held here rather than in Rust. */
  loadedRows(): number {
    let rows = 0;
    for (const block of this.#shown?.blocks.values() ?? []) rows += block.length;
    return rows;
  }

  /** The list says which rows are on screen; the missing ones are asked for. */
  show(start: number, end: number): void {
    if (start !== this.#range.start) this.#heading = start > this.#range.start ? 1 : -1;
    this.#range = { start, end };
    this.#ask(this.#shown);
    this.#ask(this.#next);
  }

  /** The row of `oid`, when it is among the rows at hand. */
  loadedIndexOf(oid: string | null): number | null {
    if (!oid || !this.#shown) return null;
    for (const [index, block] of this.#shown.blocks) {
      const at = block.find(oid);
      if (at >= 0) return index * BLOCK + at;
    }
    return null;
  }

  async indexOf(oid: string): Promise<number | null> {
    const near = this.loadedIndexOf(oid);
    if (near !== null) return near;
    const shown = this.#shown;
    if (shown?.generation == null) return null;
    return graphRowOf(shown.repo, shown.generation, oid);
  }

  /** A row a key press moves to may be off the loaded blocks. */
  async entry(index: number): Promise<GraphEntry | undefined> {
    const shown = this.#shown;
    if (shown && !this.rowAt(index)) await this.#fetch(shown, Math.floor(index / BLOCK));
    return this.rowAt(index);
  }

  async load(repo: RepoId, query: CommitQuery = EMPTY_QUERY): Promise<void> {
    await this.#load(repo, query, true);
  }

  async #load(repo: RepoId, query: CommitQuery, retry: boolean): Promise<void> {
    // Asked for by work begun before the panels moved on to another repository.
    const open = repository.current?.repo;
    if (open !== undefined && open !== repo) return;
    const load = ++this.#loads;
    this.query = query;
    const fresh = walk(repo);
    // Any history on screen stays until the new one covers it, another repository's too:
    // an empty list between them is the blink R-300 removes.
    if (this.#shown && this.#shown.total > 0) {
      this.#next = fresh;
    } else {
      this.#next = null;
      this.#show(fresh);
    }
    this.error = null;
    this.skipped = [];
    this.loading = true;

    try {
      const skipped = await loadGraph(
        repo,
        (progress) => {
          if (load !== this.#loads) return;
          this.#inherit(fresh, progress.base ?? null, progress.kept ?? 0);
          fresh.generation = progress.generation;
          fresh.total = progress.total;
          fresh.complete = progress.isLast;
          if (fresh === this.#shown) this.#publish();
          this.#ask(fresh);
        },
        { ...query, visibleRefs: this.visibleRefs },
      );
      if (load === this.#loads) this.skipped = skipped ?? [];
      if (load === this.#loads && !fresh.complete && !(await this.#settle(fresh, load)) && retry) {
        return await this.#load(repo, query, false);
      }
    } catch (err) {
      if (load === this.#loads) {
        this.#next = null;
        this.#show(walk(repo));
        this.error = asError(err);
      }
    } finally {
      if (load === this.#loads) this.loading = false;
    }
  }

  clear(): void {
    this.#loads += 1;
    this.skipped = [];
    this.query = EMPTY_QUERY;
    this.visibleRefs = null;
    this.reveal = null;
    this.#shown = null;
    this.#next = null;
    this.loading = false;
    this.error = null;
    this.#publish();
  }

  #show(next: Walk): void {
    if (this.#shown && this.#shown.repo !== next.repo) {
      this.#range = this.#rangeOf(next);
      this.home += 1;
    }
    this.#shown = next;
    this.#heading = 0;
    if (this.#next === next) this.#next = null;
    this.#publish();
  }

  #publish(): void {
    this.total = this.#shown?.total ?? 0;
    this.complete = this.#shown?.complete ?? false;
    this.#arrived += 1;
  }

  /** Blocks of the history on screen that the new walk repeats row for row (R-301). */
  #inherit(of: Walk, base: number | null, kept: number): void {
    const shown = this.#shown;
    if (base === null || !shown || shown === of || shown.repo !== of.repo || shown.generation !== base) return;
    for (const [index, block] of shown.blocks) {
      if (!of.blocks.has(index) && index * BLOCK + block.length <= kept) of.blocks.set(index, block);
    }
  }

  /** Where `of` will be on screen: another repository's history opens at its top. */
  #rangeOf(of: Walk): { start: number; end: number } {
    if (!this.#shown || this.#shown.repo === of.repo) return this.#range;
    return { start: 0, end: this.#range.end - this.#range.start };
  }

  /** Blocks around the screen that are missing, or were cut short by the walk. */
  #wanted(of: Walk): number[] {
    const range = this.#rangeOf(of);
    const end = Math.min(range.end + AHEAD, of.total);
    const wanted: number[] = [];
    for (let index = Math.floor(Math.max(range.start - AHEAD, 0) / BLOCK); index * BLOCK < end; index++) {
      const expected = Math.min(BLOCK, of.total - index * BLOCK);
      if ((of.blocks.get(index)?.length ?? 0) < expected) wanted.push(index);
    }
    return wanted;
  }

  #ask(of: Walk | null): void {
    if (!of || of.generation === null) return;
    for (const index of this.#wanted(of)) void this.#fetch(of, index);
    if (of === this.#shown) for (const index of this.#lead(of)) void this.#fetch(of, index);
    this.#promote(of);
  }

  /** The blocks just past the wanted ones, the way the list is moving; none at rest. */
  #lead(of: Walk): number[] {
    if (this.#heading === 0) return [];
    const { start, end } = this.#range;
    const first = Math.floor(Math.max(start - AHEAD, 0) / BLOCK);
    const last = Math.floor((Math.min(end + AHEAD, of.total) - 1) / BLOCK);
    const lead: number[] = [];
    for (let step = 1; step <= LEAD; step++) {
      const index = this.#heading > 0 ? last + step : first - step;
      if (index < 0 || index * BLOCK >= of.total) break;
      const expected = Math.min(BLOCK, of.total - index * BLOCK);
      if ((of.blocks.get(index)?.length ?? 0) < expected) lead.push(index);
    }
    return lead;
  }

  async #fetch(of: Walk, index: number): Promise<void> {
    if (of.generation === null) return;
    const pending = of.asking.get(index);
    if (pending) return await pending;
    const request = this.#window(of, index);
    of.asking.set(index, request);
    await request;
  }

  async #window(of: Walk, index: number): Promise<void> {
    if (of.generation === null) return;
    let served: number;
    try {
      const block = await graphWindow(of.repo, of.generation, index * BLOCK, BLOCK);
      if (!block) return;
      of.blocks.set(index, block);
      served = block.total;
      this.#adopt(of, block);
      this.#evict(of);
    } catch (err) {
      if (of === this.#shown) this.error = asError(err);
      return;
    } finally {
      of.asking.delete(index);
    }
    if (of === this.#shown) this.#arrived += 1;
    // The walk went on while this block was on its way; a reload catching up needs it too.
    const live = of === this.#shown || of === this.#next;
    if (live && served < of.total && this.#wanted(of).includes(index)) void this.#fetch(of, index);
    this.#promote(of);
  }

  /** A window counts the rows Rust had when it cut them: the count never lags the rows. */
  #adopt(of: Walk, block: GraphBlock): void {
    if (block.total <= of.total && (of.complete || !block.complete)) return;
    of.total = Math.max(of.total, block.total);
    of.complete ||= block.complete;
    if (of === this.#shown) this.#publish();
  }

  /** The walk ended without its last count. Rust's own figure, or false once it is gone. */
  async #settle(of: Walk, load: number): Promise<boolean> {
    if (of.generation === null) return false;
    const block = await graphWindow(of.repo, of.generation, 0, 0).catch(() => null);
    if (load !== this.#loads) return true;
    if (!block) return false;
    this.#adopt(of, block);
    this.#ask(of);
    return true;
  }

  /** A reload takes over once it has every row on screen, or has no more to give. */
  #promote(of: Walk): void {
    if (of !== this.#next) return;
    const { end } = this.#rangeOf(of);
    const long = of.complete || of.total >= end;
    if (long && this.#wanted(of).every((index) => index * BLOCK >= end)) this.#show(of);
  }

  #evict(of: Walk): void {
    if (of.blocks.size <= KEEP) return;
    const middle = (this.#range.start + this.#range.end) / 2 / BLOCK;
    const farthest = [...of.blocks.keys()].sort((a, b) => Math.abs(b - middle) - Math.abs(a - middle));
    for (const index of farthest.slice(0, of.blocks.size - KEEP)) of.blocks.delete(index);
  }
}

export const graph = new GraphStore();
