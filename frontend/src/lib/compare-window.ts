import type { CompareRequest } from "$lib/compare-params";
import type { DiffSpec, RepoId, Whitespace } from "$lib/ipc";

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
