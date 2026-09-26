import type { CompareRequest } from "$lib/compare-params";
import { commitDetails, type DiffSpec, type RepoId, type Whitespace } from "$lib/ipc";

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
