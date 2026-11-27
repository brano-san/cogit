import { parentFolder } from "$lib/clone";
import { ignoreFilterCase, type FilterCase } from "$lib/filter-case";
import type { ScanHit } from "$lib/ipc";
import { pathKey } from "$lib/welcome";

export type ScanLayout = "folders" | "flat";
type SectionKind = "bare" | "open" | null;

export interface ScanSection {
  id: string;
  title: string | null;
  kind: SectionKind;
  hits: readonly ScanHit[];
}

type ScanGroup = ScanSection & { title: string };

export type ScanRow =
  | { id: string; kind: "group"; section: ScanGroup; expanded: boolean }
  | { id: string; kind: "repo"; hit: ScanHit; parent: string | null };

const folderOrder = new Intl.Collator(undefined, { sensitivity: "base" });
const COLLAPSE_AT = 10;

export function scanSections(hits: readonly ScanHit[], layout: ScanLayout): ScanSection[] {
  const buckets = { normal: [] as ScanHit[], bare: [] as ScanHit[], open: [] as ScanHit[] };
  for (const hit of hits) buckets[hit.alreadyOpen ? "open" : hit.bare ? "bare" : "normal"].push(hit);

  if (layout === "flat") {
    return [{ id: "flat", title: null, kind: null, hits: [...buckets.normal, ...buckets.bare, ...buckets.open] }];
  }

  const sections: ScanSection[] = [];
  for (const kind of ["normal", "bare", "open"] as const) {
    const folders = new Map<string, ScanGroup & { hits: ScanHit[] }>();
    for (const hit of buckets[kind]) {
      const title = parentFolder(hit.root) || hit.root;
      const id = kind + ":" + pathKey(title);
      let group = folders.get(id);
      if (!group) {
        group = { id, title, kind: kind === "normal" ? null : kind, hits: [] };
        folders.set(id, group);
      }
      group.hits.push(hit);
    }
    sections.push(...[...folders.values()].sort((a, b) => folderOrder.compare(a.title, b.title)));
  }
  return sections;
}

export function scanRows(
  sections: readonly ScanSection[],
  expansion: ReadonlyMap<string, boolean>,
  filtering: boolean,
): ScanRow[] {
  const rows: ScanRow[] = [];
  for (const section of sections) {
    const { id, title, hits } = section;
    if (title !== null) {
      const expanded = expansion.get(id) ?? (filtering || hits.length < COLLAPSE_AT);
      rows.push({ id, kind: "group", section: { ...section, title }, expanded });
      if (!expanded) continue;
    }
    for (const hit of hits) {
      rows.push({ id: "repo:" + hit.root, kind: "repo", hit, parent: title === null ? null : id });
    }
  }
  return rows;
}

/** A range follows the display order and only crosses rows the user can select. */
export function scanRange(rows: readonly ScanRow[], anchor: ScanHit | null, target: ScanHit): string[] {
  const first = rows.findIndex((row) => row.kind === "repo" && row.hit === anchor);
  const last = rows.findIndex((row) => row.kind === "repo" && row.hit === target);
  if (first < 0 || last < 0) return [target.root];
  return rows.slice(Math.min(first, last), Math.max(first, last) + 1).flatMap((row) =>
    row.kind === "repo" && !row.hit.alreadyOpen ? [row.hit.root] : [],
  );
}

export function scanFilter(query: string, mode: FilterCase) {
  const needle = query.trim();
  const flags = ignoreFilterCase(needle, mode) ? "iu" : "u";
  const literal = (text: string) => text.replace(/[$.*+?^{}()|[\]\\]/g, "\\$&");
  const name = new RegExp(literal(needle), flags);
  // The discovery API emits slash paths. Accept pasted backslashes and highlight either spelling.
  const path = new RegExp(needle.split(/[\\/]/).map(literal).join("[/\\\\]"), flags);
  const isPath = (value: string) => /^([a-z]:|[\\/]{2})/i.test(value);

  return {
    active: needle !== "",
    matches: (hit: ScanHit) => name.test(hit.name) || (isPath(hit.root) ? path : name).test(hit.root),
    parts(value: string, pathLabel = false, root = value) {
      const parts: { text: string; match: boolean }[] = [];
      if (!needle) return [{ text: value, match: false }];

      // In tree view the path and the repository name are drawn on separate rows.
      // Clip a full-path match to the piece represented by this label.
      const position = pathLabel ? 0 : root.length - value.length;
      const sharesPath = root.slice(position, position + value.length) === value;
      const source = sharesPath ? root : value;
      const offset = sharesPath ? position : 0;
      const pattern = (pathLabel || source !== value) && isPath(source) ? path : name;
      let end = 0;
      for (const match of source.matchAll(new RegExp(pattern, flags + "g"))) {
        const from = Math.max(0, match.index - offset);
        const to = Math.min(value.length, match.index + match[0].length - offset);
        if (from >= to) continue;
        if (from > end) parts.push({ text: value.slice(end, from), match: false });
        parts.push({ text: value.slice(from, to), match: true });
        end = to;
      }
      if (end < value.length) parts.push({ text: value.slice(end), match: false });
      return parts;
    },
  };
}
