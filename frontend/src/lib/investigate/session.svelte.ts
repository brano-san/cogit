import type {
  BlameTables,
  FileRevision,
  OriginQuery,
  OriginReport,
} from "$lib/ipc/investigate";
import { blockAt, changeBlocks, nextChange, originQuery, type Block } from "./blame";
import {
  canGoBack,
  canGoForward,
  current,
  goBack,
  goForward,
  goTo,
  pushLocation,
  replaceCurrent,
  startHistory,
  type History,
} from "./history";
import { flatten, itemFor, makeSection, neighbour, type Section } from "./navigation";
import type { Location } from "./params";
import { perspectiveAfterSearch, searchesOrigins, type Perspective } from "./perspectives";

/** What the window needs from the backend; the tests hand in a fake. */
export interface InvestigateBackend {
  log(path: string, rev: string | null, follow: boolean): Promise<FileRevision[]>;
  blame(path: string, rev: string | null, ignoreWhitespace: boolean): Promise<BlameTables>;
  origins(query: OriginQuery, onStarted: (id: number) => void): Promise<OriginReport | null>;
  cancel(id: number): Promise<void>;
}

export type SearchState = "idle" | "searching" | "done" | "cancelled" | "failed";

function describe(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** One Investigate window: where it is, what it shows, and the search running for it. */
export class InvestigateSession {
  history = $state.raw<History>(startHistory({ path: "", rev: null, line: null }));
  perspective = $state<Perspective>("blame");
  follow = $state(true);
  ignoreWhitespace = $state(false);
  sections = $state.raw<Section[]>([]);
  logError = $state<string | null>(null);

  blame = $state.raw<BlameTables | null>(null);
  /** The file and version `blame` belongs to, also when it failed to load. */
  blameOf = $state.raw<{ path: string; rev: string | null } | null>(null);
  blameError = $state<string | null>(null);
  loadingBlame = $state(false);

  search = $state<SearchState>("idle");
  searchError = $state<string | null>(null);
  /** The block the running or finished search is about. */
  query = $state.raw<OriginQuery | null>(null);
  block = $state.raw<Block | null>(null);
  report = $state.raw<OriginReport | null>(null);
  chosen = $state(0);
  cardOpen = $state(false);

  readonly items = $derived(flatten(this.sections));
  readonly location = $derived(current(this.history));
  readonly selectedItem = $derived(itemFor(this.items, this.location.path, this.location.rev));
  readonly selectedLine = $derived(this.location.line === null ? null : this.location.line - 1);
  readonly changes = $derived(this.blame ? changeBlocks(this.blame, this.location.rev) : []);
  readonly candidate = $derived(this.report?.candidates[this.chosen] ?? null);
  readonly canGoBack = $derived(canGoBack(this.history));
  readonly canGoForward = $derived(canGoForward(this.history));

  #backend: InvestigateBackend;
  #blameGeneration = 0;
  #searchGeneration = 0;
  #searchId: number | null = null;

  constructor(backend: InvestigateBackend, start: Location) {
    this.#backend = backend;
    this.history = startHistory(start);
  }

  /** A starting version is read as the newest change to the file at or before it, so
      Navigation has a row to select even for `<commit>^`. */
  async start(): Promise<void> {
    const start = this.location;
    const rows = await this.#log(start.path, start.rev);
    const version = start.rev === null ? undefined : rows[0];
    const location = version
      ? { ...start, path: version.path || start.path, rev: version.oid }
      : start;
    this.history = startHistory(location);
    this.sections = [makeSection(location.path, start.rev, rows, start.rev === null)];
    await this.#show(location);
  }

  /** A new step: Go Deeper, a candidate, a row in Navigation. A version that only names
      a commit is read as the newest change to the file at or before it. */
  async navigate(target: Location): Promise<void> {
    let location = target;
    if (target.rev !== null && itemFor(this.items, target.path, target.rev) < 0) {
      const rows = await this.#log(target.path, target.rev);
      const version = rows[0];
      if (version) {
        location = { ...target, path: version.path || target.path, rev: version.oid };
        if (itemFor(this.items, location.path, location.rev) < 0) {
          this.sections = [...this.sections, makeSection(location.path, target.rev, rows, false)];
        }
      }
    }
    this.history = pushLocation(this.history, location);
    await this.#show(location);
  }

  async selectItem(index: number): Promise<void> {
    const item = this.items[index];
    if (!item || item.kind === "header") return;
    const rev = item.kind === "commit" ? item.row.oid : null;
    const path = item.kind === "commit" ? item.row.path || item.path : item.path;
    await this.navigate({ path, rev, line: null });
  }

  selectLine(index: number): void {
    this.history = replaceCurrent(this.history, { ...this.location, line: index + 1 });
    if (searchesOrigins(this.perspective)) void this.#searchOrigins(index);
  }

  async goDeeper(index = this.chosen): Promise<void> {
    const target = this.report?.candidates[index]?.deeper;
    if (!target) return;
    await this.navigate({ path: target.path, rev: target.rev, line: target.line });
  }

  async back(): Promise<void> {
    if (!this.canGoBack) return;
    this.history = goBack(this.history);
    await this.#show(this.location);
  }

  async forward(): Promise<void> {
    if (!this.canGoForward) return;
    this.history = goForward(this.history);
    await this.#show(this.location);
  }

  async jump(index: number): Promise<void> {
    this.history = goTo(this.history, index);
    await this.#show(this.location);
  }

  setPerspective(perspective: Perspective): void {
    this.perspective = perspective;
    const line = this.selectedLine;
    if (searchesOrigins(perspective) && line !== null && this.search === "idle") {
      void this.#searchOrigins(line);
    }
  }

  choose(index: number): void {
    if (this.report && index >= 0 && index < this.report.candidates.length) this.chosen = index;
  }

  closeCard(): void {
    this.cardOpen = false;
  }

  async step(direction: 1 | -1): Promise<void> {
    const next = neighbour(this.items, this.selectedItem, direction);
    if (next !== null) await this.selectItem(next);
  }

  moveToChange(direction: 1 | -1): void {
    const next = nextChange(this.changes, this.selectedLine ?? -1, direction);
    if (next !== null) this.selectLine(next);
  }

  async setFollow(follow: boolean): Promise<void> {
    this.follow = follow;
    await this.#reloadSections();
  }

  async setIgnoreWhitespace(ignore: boolean): Promise<void> {
    this.ignoreWhitespace = ignore;
    this.blameOf = null;
    await this.#show(this.location);
  }

  async refresh(): Promise<void> {
    this.blameOf = null;
    await this.#reloadSections();
    await this.#show(this.location);
  }

  dispose(): void {
    this.#cancelSearch();
  }

  /** Only the newest re-read writes the sections. */
  #sectionReads = 0;

  async #reloadSections(): Promise<void> {
    const read = ++this.#sectionReads;
    const before = this.sections;
    const reloaded = await Promise.all(
      before.map(async (section) =>
        makeSection(section.path, section.start, await this.#log(section.path, section.start), section.workingTree),
      ),
    );
    if (read !== this.#sectionReads) return;
    // A new start replaced the sections meanwhile: these rows belong to none of them.
    if (!before.every((section, index) => this.sections[index] === section)) return;
    // A section a navigation added while this read was on its way is newer than it.
    this.sections = [...reloaded, ...this.sections.slice(before.length)];
  }

  async #log(path: string, rev: string | null): Promise<FileRevision[]> {
    try {
      this.logError = null;
      return await this.#backend.log(path, rev, this.follow);
    } catch (err) {
      this.logError = describe(err);
      return [];
    }
  }

  async #show(location: Location): Promise<void> {
    const shown = this.blameOf;
    if (!shown || shown.path !== location.path || shown.rev !== location.rev) {
      if (!(await this.#loadBlame(location.path, location.rev))) return;
    } else {
      // What is on screen is the answer; a load still running for elsewhere is not.
      this.#blameGeneration += 1;
      this.loadingBlame = false;
    }
    if (location.line !== null && searchesOrigins(this.perspective)) {
      void this.#searchOrigins(location.line - 1);
    } else {
      this.#resetSearch();
    }
  }

  /** `false` when a newer load overtook this one and its answer was dropped. */
  async #loadBlame(path: string, rev: string | null): Promise<boolean> {
    const generation = ++this.#blameGeneration;
    this.loadingBlame = true;
    this.blameError = null;
    this.#resetSearch();
    try {
      const tables = await this.#backend.blame(path, rev, this.ignoreWhitespace);
      if (generation !== this.#blameGeneration) return false;
      this.blame = tables;
    } catch (err) {
      if (generation !== this.#blameGeneration) return false;
      this.blame = null;
      this.blameError = describe(err);
    }
    this.blameOf = { path, rev };
    this.loadingBlame = false;
    return true;
  }

  async #searchOrigins(index: number): Promise<void> {
    const query = this.blame ? originQuery(this.blame, index) : null;
    if (!query || !this.blame) return;
    this.#cancelSearch();
    const generation = ++this.#searchGeneration;
    this.query = query;
    this.block = blockAt(this.blame, index);
    this.search = "searching";
    this.searchError = null;
    this.report = null;
    this.cardOpen = true;
    try {
      const report = await this.#backend.origins(query, (id) => {
        if (generation === this.#searchGeneration) this.#searchId = id;
        else void this.#backend.cancel(id);
      });
      if (generation !== this.#searchGeneration) return;
      this.#searchId = null;
      if (!report) {
        this.search = "cancelled";
        return;
      }
      this.report = report;
      this.chosen = report.best;
      this.search = "done";
      this.perspective = perspectiveAfterSearch(this.perspective, report);
    } catch (err) {
      if (generation !== this.#searchGeneration) return;
      this.#searchId = null;
      this.search = "failed";
      this.searchError = describe(err);
    }
  }

  #cancelSearch(): void {
    if (this.#searchId !== null) void this.#backend.cancel(this.#searchId);
    this.#searchId = null;
  }

  #resetSearch(): void {
    this.#cancelSearch();
    this.#searchGeneration += 1;
    this.search = "idle";
    this.report = null;
    this.query = null;
    this.block = null;
    this.cardOpen = false;
  }
}
