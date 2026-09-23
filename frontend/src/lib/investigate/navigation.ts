import type { FileRevision } from "$lib/ipc/investigate";
import { layoutGraph, type GraphRow } from "./graph";

/** One file's log under its own header node; files reached by Go Deeper add their own.
    `start` is where the log was read from (`null` is HEAD). */
export interface Section {
  path: string;
  start: string | null;
  rows: FileRevision[];
  graph: GraphRow[];
  workingTree: boolean;
}

export type NavItem =
  | { kind: "header"; section: number; path: string }
  | { kind: "workingTree"; section: number; path: string }
  | { kind: "commit"; section: number; path: string; row: FileRevision; graph: GraphRow };

export function makeSection(
  path: string,
  start: string | null,
  rows: FileRevision[],
  workingTree: boolean,
): Section {
  return { path, start, rows, graph: layoutGraph(rows), workingTree };
}

export function flatten(sections: readonly Section[]): NavItem[] {
  const items: NavItem[] = [];
  sections.forEach((section, index) => {
    items.push({ kind: "header", section: index, path: section.path });
    if (section.workingTree) items.push({ kind: "workingTree", section: index, path: section.path });
    section.rows.forEach((row, at) => {
      items.push({
        kind: "commit",
        section: index,
        path: section.path,
        row,
        graph: section.graph[at]!,
      });
    });
  });
  return items;
}

/** The row showing `path` at `rev`, or -1. The working tree is `rev === null`. */
export function itemFor(items: readonly NavItem[], path: string, rev: string | null): number {
  if (rev === null) {
    return items.findIndex((item) => item.kind === "workingTree" && item.path === path);
  }
  const exact = items.findIndex(
    (item) => item.kind === "commit" && item.row.oid === rev && item.row.path === path,
  );
  return exact >= 0 ? exact : items.findIndex((item) => item.kind === "commit" && item.row.oid === rev);
}

/** The row of the same section one step newer (`-1`) or older (`1`), if there is one. */
export function neighbour(items: readonly NavItem[], index: number, step: 1 | -1): number | null {
  const from = items[index];
  if (!from || from.kind === "header") return null;
  const next = items[index + step];
  if (!next || next.kind === "header" || next.section !== from.section) return null;
  return index + step;
}
