import { graphOverlay, type GraphOverlay, type GraphPaintRequest, type RepoId } from "$lib/ipc";
import { rowPaint, type RowPaint } from "$lib/graph-style";

/** The graph store's block: a window of paint lines up with a window of rows. */
const BLOCK = 128;
const KEEP = 48;
/** While the walk goes on a later row can recolour one on screen; asked again this often. */
const REFRESH_MS = 300;

export type FetchOverlay = (
  repo: RepoId,
  generation: number,
  start: number,
  count: number,
  request: GraphPaintRequest,
) => Promise<GraphOverlay | null>;

export interface OverlayView {
  repo: RepoId | null;
  /** Rust's number for the walk on screen; `null` before its first progress message. */
  generation: number | null;
  /** Commit rows on screen, `end` excluded. */
  start: number;
  end: number;
  /** Rows laid out so far, and whether that is all of them. */
  total: number;
  complete: boolean;
  request: GraphPaintRequest | null;
}

/** Paint of the rows on screen (#11, #26), fetched by block beside the rows themselves.
    Anything that changes the picture — another walk, another request — starts over. */
export class GraphOverlayStore {
  #arrived = $state(0);
  #key = "";
  #view: OverlayView | null = null;
  #blocks = new Map<number, GraphOverlay>();
  #asking = new Set<number>();
  #asked = new Map<number, number>();
  #timer: ReturnType<typeof setTimeout> | null = null;
  readonly #fetch: FetchOverlay;
  readonly #now: () => number;

  constructor(fetch: FetchOverlay = graphOverlay, now: () => number = () => Date.now()) {
    this.#fetch = fetch;
    this.#now = now;
  }

  paintAt(row: number): RowPaint | undefined {
    void this.#arrived;
    const block = this.#blocks.get(Math.floor(row / BLOCK));
    return block ? rowPaint(block, row) : undefined;
  }

  show(view: OverlayView): void {
    const key = JSON.stringify([view.repo, view.generation, view.request]);
    if (key !== this.#key) {
      this.#key = key;
      this.#blocks = new Map();
      this.#asking.clear();
      this.#asked.clear();
      this.#arrived += 1;
    }
    this.#view = view;
    this.#fill();
  }

  clear(): void {
    this.show({ repo: null, generation: null, start: 0, end: 0, total: 0, complete: false, request: null });
  }

  #fill(): void {
    const view = this.#view;
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    if (!view?.request || view.repo === null || view.generation === null) return;
    const end = Math.min(view.end, view.total);
    let wait = Infinity;
    for (let index = Math.floor(view.start / BLOCK); index * BLOCK < end; index++) {
      const have = this.#blocks.get(index);
      if (this.#asking.has(index) || (have && have.total >= view.total)) continue;
      const since = this.#now() - (this.#asked.get(index) ?? -Infinity);
      if (have && !view.complete && since < REFRESH_MS) {
        wait = Math.min(wait, REFRESH_MS - since);
        continue;
      }
      void this.#get(this.#key, view, index);
    }
    if (wait !== Infinity) this.#timer = setTimeout(() => this.#fill(), wait);
    this.#evict(view);
  }

  async #get(key: string, view: OverlayView, index: number): Promise<void> {
    if (!view.request || view.repo === null || view.generation === null) return;
    this.#asking.add(index);
    this.#asked.set(index, this.#now());
    let overlay: GraphOverlay | null = null;
    try {
      overlay = await this.#fetch(view.repo, view.generation, index * BLOCK, BLOCK, view.request);
    } catch {
      // Paint is decoration: the rows are drawn in the default colours without it.
    }
    if (key !== this.#key) return;
    this.#asking.delete(index);
    if (!overlay) return;
    this.#blocks.set(index, overlay);
    this.#arrived += 1;
    this.#fill();
  }

  #evict(view: OverlayView): void {
    if (this.#blocks.size <= KEEP) return;
    const middle = (view.start + view.end) / 2 / BLOCK;
    const farthest = [...this.#blocks.keys()].sort((a, b) => Math.abs(b - middle) - Math.abs(a - middle));
    for (const index of farthest.slice(0, this.#blocks.size - KEEP)) this.#blocks.delete(index);
  }
}

export const graphOverlays = new GraphOverlayStore();
