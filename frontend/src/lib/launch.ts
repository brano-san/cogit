import { droppedRepositories, openDropped, type DropHost } from "$lib/drop-open";
import type { LaunchRequest, RepoId } from "$lib/ipc";
import { pathKey } from "$lib/welcome";

/** `path` as the repository at `root` names it (`/`, relative), or `null` when it is not
    inside. Drive letters and UNC paths compare without case, as Windows does. */
export function pathInRepo(root: string, path: string): string | null {
  const slashed = (text: string) => text.replace(/\\/g, "/").replace(/\/+$/, "");
  const [base, full] = [slashed(root), slashed(path)];
  const [baseKey, fullKey] = [pathKey(base), pathKey(full)];
  if (!fullKey.startsWith(`${baseKey}/`)) return null;
  return full.slice(base.length + 1);
}

export interface LaunchHost extends DropHost {
  /** The full open that takes the panels; the repository's root, or `null` when it failed. */
  activate: (path: string) => Promise<string | null>;
  shown: () => RepoId | null;
  investigate: (repo: RepoId, path: string) => Promise<void>;
  blame: (repo: RepoId, path: string) => Promise<void>;
}

/** `cogit <path>…` opens like folders dropped on the window; `cogit log`/`blame` opens the
    repository, then the file's window. A folder has no log of its own: its graph is it. */
export async function runLaunch(request: LaunchRequest, host: LaunchHost): Promise<void> {
  if (request.show === "repository") {
    await openDropped(droppedRepositories(request.paths.map((each) => each.folder)), host);
    return;
  }
  const target = request.paths[0];
  if (!target) return;
  const isFile = target.folder !== target.path;
  if (request.show === "blame" && !isFile) {
    host.report(new Error(`${target.path} is not a file.`), "Could not open Blame");
    return;
  }
  const root = await host.activate(target.folder);
  const repo = host.shown();
  if (root === null || repo === null || !isFile) return;
  const inside = pathInRepo(root, target.path);
  if (inside === null) {
    host.report(new Error(`${target.path} is not inside ${root}.`), "Could not open the file");
    return;
  }
  if (request.show === "log") {
    await host.investigate(repo, inside).catch((err) => host.report(err, "Could not open Investigate"));
  } else {
    await host.blame(repo, inside).catch((err) => host.report(err, "Could not open Blame"));
  }
}
