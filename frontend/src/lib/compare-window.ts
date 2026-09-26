import type { CompareRequest } from "$lib/compare-params";
import { closeThisWindow, commitDetails, type DiffSpec, type RepoId, type Whitespace } from "$lib/ipc";
import { askToOpenModule, type ModuleRequest } from "$lib/module-open";

export interface CompareLoad {
  settings: { load(): Promise<void>; current: { ignoreWhitespace: Whitespace } };
  diff: { whitespace: Whitespace; load(repo: RepoId, spec: DiffSpec, path: string): Promise<void> };
}

/** Preferences first: the diff takes its context lines and whitespace from them, and
    started beside the settings read it took the defaults. */
export async function loadCompare(request: CompareRequest, { settings, diff }: CompareLoad): Promise<void> {
  await settings.load();
  diff.whitespace = settings.current.ignoreWhitespace;
  await diff.load(request.repo, request.spec, request.path);
}

/** The first parent a `commitVsParent` window compares against, for its header: `null` for a
    root commit, `undefined` for any other comparison or when it cannot be read. */
export async function firstParent(
  request: CompareRequest,
  read: (repo: RepoId, rev: string) => Promise<{ parents: string[] }> = commitDetails,
): Promise<string | null | undefined> {
  if (request.spec.kind !== "commitVsParent") return undefined;
  try {
    return (await read(request.repo, request.spec.oid)).parents[0] ?? null;
  } catch {
    return undefined;
  }
}

/** A submodule has no lines to compare: the main window opens it, and this one goes (R-537). */
export async function handOverModule(
  request: CompareRequest,
  ask: (request: ModuleRequest) => Promise<void> = askToOpenModule,
  close: () => Promise<unknown> = closeThisWindow,
): Promise<void> {
  await ask({ repo: request.repo, path: request.path });
  await close();
}
