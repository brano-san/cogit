import { defaultVisible, type RefNode } from "$lib/ref-nodes";
import { parseRefSort, type RefSort } from "$lib/ref-sort";
import { recall, remember } from "$lib/session-memory";
import { listRemotes, refDates, remoteUrl, type RepoId } from "$lib/ipc";

/** Per repository, so ticking `master` in one does not change what another shows. */
const STORAGE_KEY = "cogit.visible-refs.v2";
/** One order for every repository: it is how the user reads lists, not a repository fact. */
const SORT_KEY = "cogit.ref-sort.v1";

function storedSort(): RefSort {
  try {
    const raw = localStorage.getItem(SORT_KEY);
    return parseRefSort(raw === null ? null : JSON.parse(raw));
  } catch {
    return parseRefSort(null);
  }
}

interface Saved {
  visible: string[];
}

function foldableIds(nodes: readonly RefNode[]): Set<string> {
  return new Set(nodes.filter((node) => node.children === true).map((node) => node.id));
}

function stored(): Record<string, Saved> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? {} : (JSON.parse(raw) as Record<string, Saved>);
  } catch {
    return {};
  }
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry) => typeof entry === "string") : [];
}

class RefsStore {
  visible = $state.raw<ReadonlySet<string>>(new Set());
  filter = $state("");
  urls = $state.raw<Record<string, string>>({});
  sort = $state.raw<RefSort>(storedSort());
  /** Tip dates by full ref name, read only while the sort goes by date. */
  dates = $state.raw<ReadonlyMap<string, number>>(new Map());

  #root: string | null = null;
  #expanded = $state.raw<ReadonlySet<string>>(new Set());
  #foldable = $state.raw<ReadonlySet<string>>(new Set());

  get collapsed(): ReadonlySet<string> {
    return new Set([...this.#foldable].filter((id) => !this.#expanded.has(id)));
  }

  /** Until the tree has been built once there is nothing to seed a default from. */
  adopt(root: string, nodes: readonly RefNode[]): void {
    if (this.#root === root) return;
    this.#root = root;
    const remembered = stored()[root];
    const known = new Set(nodes.map((node) => node.id));
    const kept = strings(remembered?.visible).filter((id) => known.has(id));
    this.visible = kept.length > 0 ? new Set(kept) : defaultVisible(nodes);
    this.#foldable = foldableIds(nodes);
    this.#expanded = recall("refs", root);
  }

  /** The tree grows after the open — stashes, lost commits, a fetched remote — and a
      heading nobody has opened yet arrives folded. */
  know(nodes: readonly RefNode[]): void {
    const next = foldableIds(nodes);
    const same =
      next.size === this.#foldable.size && [...next].every((id) => this.#foldable.has(id));
    if (!same) this.#foldable = next;
  }

  set(next: Set<string>): void {
    this.visible = next;
    this.persist();
  }

  /** Folds an open heading and opens a folded one. */
  collapse(id: string): void {
    const next = new Set(this.#expanded);
    if (!next.delete(id)) next.add(id);
    this.#expanded = next;
    if (this.#root !== null) remember("refs", this.#root, next);
  }

  private persist(): void {
    if (this.#root === null) return;
    try {
      const all = stored();
      all[this.#root] = { visible: [...this.visible] };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
    } catch {
      // A blocked store costs the ticks and the folds on restart, nothing more.
    }
  }

  setSort(next: RefSort): void {
    this.sort = next;
    try {
      localStorage.setItem(SORT_KEY, JSON.stringify(next));
    } catch {
      // Kept for this run; the next one starts from the default order.
    }
  }

  /** A failed read leaves the old dates: a stale order beats a list that jumps to by-name. */
  async loadDates(repo: RepoId): Promise<void> {
    try {
      const found = await refDates(repo);
      this.dates = new Map(found.map((entry) => [entry.fullName, entry.timestamp]));
    } catch {
      // Nothing to report; the names still order what has no date.
    }
  }

  async loadUrls(repo: RepoId): Promise<void> {
    try {
      const names = await listRemotes(repo);
      const pairs = await Promise.all(
        names.map(async (name) => [name, (await remoteUrl(repo, name)) ?? ""] as const),
      );
      this.urls = Object.fromEntries(pairs.filter(([, url]) => url !== ""));
    } catch {
      this.urls = {};
    }
  }

  clear(): void {
    this.#root = null;
    this.visible = new Set();
    this.#expanded = new Set();
    this.#foldable = new Set();
    this.filter = "";
    this.urls = {};
    this.dates = new Map();
  }
}

export const refs = new RefsStore();
