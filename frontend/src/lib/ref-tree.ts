import type { Branch } from "$lib/ipc";

export interface RefRow {
  label: string;
  depth: number;
  branch?: Branch;
}

export function matchesFilter(name: string, filter: string): boolean {
  const needle = filter.trim().toLowerCase();
  return needle === "" || name.toLowerCase().includes(needle);
}

/** `feature/auth/login` becomes three rows so long prefixes are written once. */
export function buildTree(branches: readonly Branch[]): RefRow[] {
  const rows: RefRow[] = [];
  const seen = new Set<string>();

  for (const branch of branches) {
    const parts = branch.name.split("/").filter((part) => part !== "");
    parts.forEach((part, depth) => {
      const isLeaf = depth === parts.length - 1;
      const path = parts.slice(0, depth + 1).join("/");
      if (!isLeaf) {
        if (seen.has(path)) return;
        seen.add(path);
        rows.push({ label: part, depth });
        return;
      }
      rows.push({ label: part, depth, branch });
    });
  }
  return rows;
}
