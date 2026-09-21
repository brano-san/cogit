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
