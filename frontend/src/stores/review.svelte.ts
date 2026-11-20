import { commitFiles, type CogitError, type DiffSpec, type FileEntry, type RepoId, toCogitError } from "$lib/ipc";
import {
  branchReview,
  reviewBase,
  reviewFiles,
  reviewLineCounts,
  type BranchReview,
  type LineCounts,
  type ReviewPath,
} from "$lib/ipc/review";
import { shortOid } from "$lib/format";

export interface ReviewIpc {
  reviewBase(repo: RepoId, branch: string): Promise<string | null>;
  branchReview(repo: RepoId, base: string, branch: string): Promise<BranchReview>;
  reviewFiles(
    repo: RepoId,
    mergeBase: string,
    branch: string,
    worktree: boolean,
    onchunk: (files: FileEntry[]) => void,
  ): Promise<number>;
  reviewLineCounts(repo: RepoId, spec: DiffSpec, files: ReviewPath[]): Promise<LineCounts[]>;
  commitFiles(repo: RepoId, oid: string): Promise<FileEntry[]>;
}

const IPC: ReviewIpc = { reviewBase, branchReview, reviewFiles, reviewLineCounts, commitFiles };

export const COUNT_BATCH = 100;

export const NO_BASE = "Cogit found no main line to review against (the remote's default branch, main or master). Choose a base.";

function specKey(spec: DiffSpec): string {
  switch (spec.kind) {
    case "commitVsParent":
      return `p:${spec.oid}`;
    case "commitVsCommit":
      return `c:${spec.a}:${spec.b}`;
    case "commitVsWorkTree":
      return `w:${spec.oid}`;
    default:
      return spec.kind;
  }
}

export function totalsOf(counts: readonly (LineCounts | undefined)[]): { added: number; removed: number; counted: number } {
  let added = 0;
  let removed = 0;
  let counted = 0;
  for (const each of counts) {
    if (!each) continue;
    counted += 1;
    added += each.added ?? 0;
    removed += each.removed ?? 0;
  }
  return { added, removed, counted };
}

export class ReviewStore {
  repo = $state.raw<RepoId | null>(null);
  branch = $state<string | null>(null);
  base = $state<string | null>(null);
  summary = $state.raw<BranchReview | null>(null);
  files = $state.raw<FileEntry[]>([]);
  commitFiles = $state.raw<FileEntry[] | null>(null);
  commit = $state<string | null>(null);
  worktree = $state(false);
  loading = $state(false);
  listing = $state(false);
  error = $state.raw<CogitError | null>(null);
  problem = $state<string | null>(null);
  selected = $state<string | null>(null);
  counts = $state.raw<ReadonlyMap<string, LineCounts>>(new Map());

  #generation = 0;
  #ipc: ReviewIpc;
  #show: (repo: RepoId, spec: DiffSpec, path: string) => void;

  constructor(show: (repo: RepoId, spec: DiffSpec, path: string) => void, ipc: ReviewIpc = IPC) {
    this.#show = show;
    this.#ipc = ipc;
  }

  get shown(): readonly FileEntry[] {
    return this.commit === null ? this.files : (this.commitFiles ?? []);
  }

  get note(): string | null {
    const bases = this.summary?.mergeBases ?? [];
    if (bases.length < 2 || !this.summary) return null;
    return `${bases.length} merge bases (criss-cross merges): the changes are shown from ${shortOid(this.summary.mergeBase)}, the one git merge-base picks.`;
  }

  get merged(): boolean {
    return this.summary !== null && this.summary.ahead === 0 && !this.listing && this.files.length === 0;
  }

  get canIncludeWorktree(): boolean {
    return this.summary?.isHead ?? false;
  }

  specFor(file: FileEntry): DiffSpec | null {
    const summary = this.summary;
    if (!summary) return null;
    if (this.commit !== null) return { kind: "commitVsParent", oid: this.commit };
    if (this.worktree && !file.oldPath) return { kind: "commitVsWorkTree", oid: summary.mergeBase };
    return { kind: "commitVsCommit", a: summary.mergeBase, b: summary.branchOid };
  }

  countsOf(file: FileEntry): LineCounts | undefined {
    const spec = this.specFor(file);
    return spec ? this.counts.get(`${specKey(spec)}\0${file.path}`) : undefined;
  }

  get totals() {
    return totalsOf(this.shown.map((file) => this.countsOf(file)));
  }

  async open(repo: RepoId, branch: string, base: string | null): Promise<void> {
    this.repo = repo;
    this.branch = branch;
    this.base = base;
    this.commit = null;
    this.commitFiles = null;
    this.worktree = false;
    this.selected = null;
    await this.reload();
  }

  async setBase(base: string): Promise<void> {
    if (base === this.base) return;
    this.base = base;
    this.commit = null;
    this.commitFiles = null;
    await this.reload();
  }

  async setWorktree(on: boolean): Promise<void> {
    if (on === this.worktree || (on && !this.canIncludeWorktree)) return;
    this.worktree = on;
    if (this.commit === null) await this.#list(++this.#generation);
  }

  async reload(): Promise<void> {
    const generation = ++this.#generation;
    const repo = this.repo;
    const branch = this.branch;
    if (repo === null || branch === null) return;
    this.loading = true;
    this.error = null;
    this.problem = null;
    this.forgetWorktreeCounts();
    try {
      if (this.base === null) this.base = await this.#ipc.reviewBase(repo, branch);
      if (generation !== this.#generation) return;
      if (this.base === null) {
        this.summary = null;
        this.files = [];
        this.problem = NO_BASE;
        return;
      }
      const summary = await this.#ipc.branchReview(repo, this.base, branch);
      if (generation !== this.#generation) return;
      this.summary = summary;
      if (!summary.isHead) this.worktree = false;
      if (this.commit !== null && !summary.commits.some((each) => each.oid === this.commit)) {
        this.commit = null;
        this.commitFiles = null;
      }
    } catch (err) {
      if (generation !== this.#generation) return;
      this.summary = null;
      this.files = [];
      this.error = toCogitError(err);
      return;
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
    await this.#list(generation);
    if (this.commit !== null && generation === this.#generation) await this.#listCommit(this.commit, generation);
  }

  async #list(generation: number): Promise<void> {
    const repo = this.repo;
    const summary = this.summary;
    if (repo === null || !summary) return;
    this.listing = true;
    let first = true;
    try {
      const total = await this.#ipc.reviewFiles(repo, summary.mergeBase, summary.branch, this.worktree, (chunk) => {
        if (generation !== this.#generation) return;
        this.files = first ? chunk : [...this.files, ...chunk];
        first = false;
      });
      if (generation !== this.#generation) return;
      if (total === 0) this.files = [];
    } catch (err) {
      if (generation !== this.#generation) return;
      this.error = toCogitError(err);
      this.files = [];
      return;
    } finally {
      if (generation === this.#generation) this.listing = false;
    }
    this.#dropSelectionIfGone();
    await this.#count(generation, this.files);
  }

  async selectCommit(oid: string | null): Promise<void> {
    const next = oid === this.commit ? null : oid;
    const generation = ++this.#generation;
    this.commit = next;
    this.commitFiles = null;
    this.listing = false;
    if (next === null) {
      this.#dropSelectionIfGone();
      await this.#count(generation, this.files);
      return;
    }
    await this.#listCommit(next, generation);
  }

  async #listCommit(oid: string, generation: number): Promise<void> {
    const repo = this.repo;
    if (repo === null) return;
    this.listing = true;
    try {
      const files = await this.#ipc.commitFiles(repo, oid);
      if (generation !== this.#generation) return;
      this.commitFiles = files;
    } catch (err) {
      if (generation !== this.#generation) return;
      this.error = toCogitError(err);
      this.commitFiles = [];
      return;
    } finally {
      if (generation === this.#generation) this.listing = false;
    }
    this.#dropSelectionIfGone();
    await this.#count(generation, this.commitFiles ?? []);
  }

  #dropSelectionIfGone(): void {
    if (this.selected === null) return;
    if (this.shown.some((file) => file.path === this.selected)) this.select(this.selected);
    else this.selected = null;
  }

  async #count(generation: number, files: readonly FileEntry[]): Promise<void> {
    const repo = this.repo;
    if (repo === null) return;
    const groups = new Map<string, { spec: DiffSpec; files: ReviewPath[] }>();
    for (const file of files) {
      const spec = this.specFor(file);
      if (!spec || file.mode === "submodule") continue;
      const key = specKey(spec);
      if (this.counts.has(`${key}\0${file.path}`)) continue;
      const group = groups.get(key) ?? { spec, files: [] };
      group.files.push({ path: file.path, oldPath: file.oldPath ?? null });
      groups.set(key, group);
    }
    for (const [key, group] of groups) {
      for (let at = 0; at < group.files.length; at += COUNT_BATCH) {
        if (generation !== this.#generation) return;
        let found: LineCounts[];
        try {
          found = await this.#ipc.reviewLineCounts(repo, group.spec, group.files.slice(at, at + COUNT_BATCH));
        } catch {
          return;
        }
        if (generation !== this.#generation) return;
        const next = new Map(this.counts);
        for (const each of found) next.set(`${key}\0${each.path}`, each);
        this.counts = next;
      }
    }
  }

  forgetWorktreeCounts(): void {
    const next = new Map([...this.counts].filter(([key]) => !key.startsWith("w:")));
    if (next.size !== this.counts.size) this.counts = next;
  }

  select(path: string): void {
    const file = this.shown.find((each) => each.path === path);
    const spec = file ? this.specFor(file) : null;
    if (!file || !spec || this.repo === null) return;
    this.selected = path;
    this.#show(this.repo, spec, path);
  }
}
