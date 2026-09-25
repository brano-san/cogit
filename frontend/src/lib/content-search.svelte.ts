import { cancelOperation, searchFileContents } from "$lib/ipc";
import type { FileEntry, RepoId, SearchChunk, SearchScope } from "$lib/ipc/bindings";
import { matches, type Pattern } from "./file-search";
import type { FileView } from "./file-view";

/** Longer than the diff find bar's pause: every search opens files on disk. */
export const SETTLE_MS = 250;

export interface ContentQuery {
  text: string;
  regex: boolean;
  scope: SearchScope;
}

/** With unchanged files listed, every file is on the list, so every file is worth reading. */
export function contentScope(view: FileView): SearchScope {
  return view.unchanged ? "all" : "changed";
}

/** `null` while the list filters by name: the switch is off or the field is empty. */
export function contentQuery(view: FileView, text: string): ContentQuery | null {
  const needle = text.trim();
  if (!view.contents || needle === "") return null;
  return { text: needle, regex: view.regex, scope: contentScope(view) };
}

function sameQuery(a: ContentQuery | null, b: ContentQuery | null): boolean {
  if (a === null || b === null) return a === b;
  return a.text === b.text && a.regex === b.regex && a.scope === b.scope;
}

/** Searching contents replaces the name filter rather than adding to it (R-270). A new
    folder is one `dir/` row: it stays when a file inside it matches. */
export function keepFile(
  file: FileEntry,
  pattern: Pattern,
  hits: ReadonlyMap<string, number> | null,
): boolean {
  if (hits === null) return matches(file, pattern);
  if (hits.has(file.path)) return true;
  if (!file.path.endsWith("/")) return false;
  for (const path of hits.keys()) if (path.startsWith(file.path)) return true;
  return false;
}

export type Runner = (
  repo: RepoId,
  query: ContentQuery,
  onChunk: (chunk: SearchChunk) => void,
) => Promise<unknown>;

export type Canceller = (id: number) => Promise<unknown>;

const runSearch: Runner = (repo, query, onChunk) =>
  searchFileContents(repo, query.text, query.regex, query.scope, onChunk);

/**
 * The Files panel's content search: debounced, one search at a time, streamed.
 *
 * The previous search keeps its results on screen until the next one reports anything,
 * so typing one more letter does not blank the list between two answers.
 */
export class ContentSearch {
  /** Path → matching lines; `null` while no content search is active. */
  hits = $state.raw<ReadonlyMap<string, number> | null>(null);
  busy = $state(false);
  error = $state<string | null>(null);

  #query: ContentQuery | null = null;
  #generation = 0;
  #operation: number | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #repo: () => RepoId | null;
  #run: Runner;
  #cancel: Canceller;

  constructor(repo: () => RepoId | null, run: Runner = runSearch, cancel: Canceller = cancelOperation) {
    this.#repo = repo;
    this.#run = run;
    this.#cancel = cancel;
  }

  set(query: ContentQuery | null): void {
    if (sameQuery(query, this.#query)) return;
    this.#query = query;
    if (query === null) {
      this.#stop();
      this.hits = null;
      this.busy = false;
      this.error = null;
      return;
    }
    this.#schedule(query);
  }

  /** The files on disk changed under the same query. */
  refresh(): void {
    if (this.#query !== null) this.#schedule(this.#query);
  }

  #schedule(query: ContentQuery): void {
    this.#stop();
    this.busy = true;
    this.#timer = setTimeout(() => void this.#start(query), SETTLE_MS);
  }

  #stop(): void {
    clearTimeout(this.#timer);
    this.#generation += 1;
    if (this.#operation !== null) void this.#cancel(this.#operation);
    this.#operation = null;
  }

  async #start(query: ContentQuery): Promise<void> {
    const repo = this.#repo();
    if (repo === null) {
      this.busy = false;
      return;
    }
    const generation = ++this.#generation;
    const found = new Map<string, number>();
    const current = () => generation === this.#generation;

    const onChunk = (chunk: SearchChunk) => {
      if (chunk.kind === "started") {
        if (current()) this.#operation = chunk.id;
        else void this.#cancel(chunk.id);
        return;
      }
      if (!current()) return;
      if (chunk.kind === "matches") {
        for (const match of chunk.matches) found.set(match.path, (found.get(match.path) ?? 0) + 1);
      } else {
        this.#operation = null;
        this.busy = false;
      }
      this.error = null;
      this.hits = new Map(found);
    };

    try {
      await this.#run(repo, query, onChunk);
    } catch (err) {
      if (!current()) return;
      this.error = err instanceof Error ? err.message : String(err);
      this.hits = new Map();
    } finally {
      if (current()) {
        this.#operation = null;
        this.busy = false;
      }
    }
  }
}
