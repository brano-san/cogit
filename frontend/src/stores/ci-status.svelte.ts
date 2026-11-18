import { SvelteMap } from "svelte/reactivity";
import { ciMenu, menuUrl, mergeTargets, nextPoll, sameTargets, type CiMenu } from "$lib/ci-status";
import { ciStatuses, popupContextMenu, type CiStatus, type CiTarget, type RepoId } from "$lib/ipc";
import { openExternal } from "$lib/ipc/file-menus";
import { errors } from "$stores/errors.svelte";

export type CiSource = "graph" | "branches";

const SETTLE_MS = 250;
const MAX_HELD = 2_000;

class CiStatusStore {
  readonly statuses = new SvelteMap<string, CiStatus>();

  #repo: RepoId | null = null;
  #windows = new Map<CiSource, CiTarget[]>();
  #asked: CiTarget[] = [];
  #settle: ReturnType<typeof setTimeout> | undefined;
  #poll: ReturnType<typeof setTimeout> | undefined;
  #running = false;
  #again = false;
  #menu: CiMenu | null = null;

  setWindow(repo: RepoId, source: CiSource, targets: readonly CiTarget[]): void {
    if (repo !== this.#repo) {
      this.clear();
      this.#repo = repo;
    }
    const held = this.#windows.get(source) ?? [];
    if (sameTargets(held, targets)) return;
    this.#windows.set(source, [...targets]);
    clearTimeout(this.#settle);
    this.#settle = setTimeout(() => void this.#ask(false), SETTLE_MS);
  }

  dropWindow(source: CiSource): void {
    this.#windows.delete(source);
  }

  async #ask(polled: boolean): Promise<void> {
    if (this.#running) {
      this.#again = true;
      return;
    }
    const repo = this.#repo;
    const targets = mergeTargets(...this.#windows.values());
    if (repo === null || targets.length === 0) return;
    if (!polled && sameTargets(targets, this.#asked)) return;
    clearTimeout(this.#poll);
    this.#poll = undefined;
    this.#asked = targets;
    this.#running = true;
    let retry: number | null = null;
    try {
      const batch = await ciStatuses(repo, targets);
      if (repo !== this.#repo) return;
      if (this.statuses.size > MAX_HELD) this.statuses.clear();
      for (const status of batch.statuses) this.statuses.set(status.sha, status);
      retry = nextPoll(batch.retryMs);
    } catch {
      retry = null;
    } finally {
      this.#running = false;
    }
    if (repo !== this.#repo) return;
    if (this.#again) {
      this.#again = false;
      void this.#ask(false);
      return;
    }
    if (retry !== null) this.#poll = setTimeout(() => void this.#ask(true), retry);
  }

  look(sha: string): CiStatus | undefined {
    return this.statuses.get(sha);
  }

  async openMenu(status: CiStatus, x: number, y: number): Promise<void> {
    this.#menu = ciMenu(status);
    await popupContextMenu(this.#menu.items, x, y).catch(() => {});
  }

  runMenu(id: string): boolean {
    const url = menuUrl(this.#menu, id);
    if (url === undefined) return false;
    if (url !== null) void openExternal(url).catch((err) => errors.report(err, "Could not open the link"));
    return true;
  }

  clear(): void {
    clearTimeout(this.#settle);
    clearTimeout(this.#poll);
    this.#settle = undefined;
    this.#poll = undefined;
    this.#repo = null;
    this.#windows = new Map();
    this.#asked = [];
    this.#again = false;
    if (this.statuses.size > 0) this.statuses.clear();
  }
}

export const ciStatus = new CiStatusStore();
