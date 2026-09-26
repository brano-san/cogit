/** Paths dropped on the window, made fit to hand to `open_repository` one at a time. */
export function droppedRepositories(paths: readonly string[], limit = 8): string[] {
  const seen = new Set<string>();
  const roots: string[] = [];
  for (const path of paths) {
    const trimmed = path.trim().replace(/[\\/]+$/, "");
    if (trimmed === "" || seen.has(trimmed)) continue;
    seen.add(trimmed);
    roots.push(trimmed);
    if (roots.length === limit) break;
  }
  return roots;
}

export interface DropHost {
  /** `open_repository` alone: listed, the panels left as they are. */
  openInList: (path: string) => Promise<unknown>;
  /** The full open that takes the panels. */
  activate: (path: string) => Promise<unknown>;
  refreshList: () => Promise<void>;
  report: (err: unknown, title: string) => void;
}

/** Every folder but the first goes into the list, as the picks of a scan do; the first
    takes the panels, so they change hands once. */
export async function openDropped(paths: readonly string[], host: DropHost): Promise<void> {
  for (const path of paths.slice(1)) {
    await host.openInList(path).catch((err) => host.report(err, "Could not open the repository"));
  }
  await host.refreshList();
  const first = paths[0];
  if (first) await host.activate(first);
}
