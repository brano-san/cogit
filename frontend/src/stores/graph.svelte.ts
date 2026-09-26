import {
  type CogitError,
  EMPTY_QUERY,
  graphRowOf,
  graphWindow,
  loadGraph,
  type CommitQuery,
  type GraphView,
  type RepoId,
  toCogitError,
} from "$lib/ipc";
import type { GraphBlock, GraphEntry } from "$lib/graph-wire";
import { repository } from "$stores/repository.svelte";
import { GRAPH_MODE_DEFAULTS, graphView } from "$lib/graph-modes";
import { LONG_LINK_ROWS } from "$lib/graph-row";

export type { GraphEntry };

/** Rows per request. A screen is about forty; the next block is asked for early. */
const BLOCK = 128;
const AHEAD = 64;
/** Blocks kept per graph; the ones farthest from the screen go first. */
const KEEP = 48;

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

  /** Graph modes that decide which commits are shown (#26), folded into every load. */
  view = $state.raw<GraphView>(graphView(GRAPH_MODE_DEFAULTS));
  /** Links longer than this many rows are drawn as two stubs (R-330); 0 draws them whole.
      Folded into every load, like the refs. */
  longLinkRows = $state(LONG_LINK_ROWS);

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
  #loads = 0;

  constructor() {
    // No load of the repository being left may take the screen or report after it, its
    // first one included: that one walks as the history on screen (R-300).
    repository.onLeave(() => {
      this.#loads += 1;
      this.#next = null;
      this.loading = false;
    });
  }

  /** The repository whose rows are on screen; it lags the one open while its graph loads. */
  get shownRepo(): RepoId | null {
    return this.#shown?.repo ?? null;
  }

  /** The rows on screen are the last repository's: nothing done to them may reach the one
      open, which does not have their commits (R-300). */
  get stale(): boolean {
    return this.#shown !== null && this.#shown.repo !== repository.current?.repo;
  }

  /** The walk on screen, for what is fetched beside its rows (paint, #11). */
  get walk(): { repo: RepoId; generation: number | null } | null {
    void this.#arrived;
    return this.#shown && { repo: this.#shown.repo, generation: this.#shown.generation };
  }

  /** The repository whose graph is loading, else the one on screen: while another
      repository's graph catches up, the rows on screen are still the last one's (R-300). */
  get #loadingRepo(): RepoId | undefined {
    return this.#next?.repo ?? this.#shown?.repo;
  }

  /** Another view walks the graph again. */
  setView(next: GraphView): void {
    if (JSON.stringify(next) === JSON.stringify(this.view)) return;
    this.view = next;
    const repo = this.#loadingRepo;
    if (repo !== undefined) void this.load(repo, this.query);
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
    this.#range = { start, end };
    this.#ask(this.#shown);
    this.#ask(this.#next);
  }

  /** The row of `oid`, when it is among the rows at hand. */
  loadedIndexOf(oid: string | null): number | null {
    void this.#arrived;
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

  /** Row and column of `oid` without keeping its block: the dashed line to HEAD reaches
      rows far below the screen, and a block kept for it would be the first evicted. */
  async locate(oid: string): Promise<{ row: number; lane: number } | null> {
    const shown = this.#shown;
    const row = await this.indexOf(oid);
    if (row === null || !shown || shown !== this.#shown) return null;
    const near = this.rowAt(row);
    if (near) return { row, lane: near.layout.lane };
    if (shown.generation === null) return null;
    const block = await graphWindow(shown.repo, shown.generation, row, 1).catch(() => null);
    return block && block.length > 0 ? { row, lane: block.entry(0).layout.lane } : null;
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
    // Asked for by work begun before the panels moved on to another repository, or closed it.
    if (repository.current?.repo !== repo) return;
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
        { ...query, visibleRefs: this.visibleRefs, view: this.view, longLinkRows: this.longLinkRows },
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

  /** A new threshold lays the shown history out again. */
  setLongLinkRows(rows: number): void {
    const next = Math.max(Math.round(rows), 0);
    if (next === this.longLinkRows) return;
    this.longLinkRows = next;
    const repo = this.#loadingRepo;
    if (repo !== undefined) void this.load(repo, this.query);
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
    this.#promote(of);
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
