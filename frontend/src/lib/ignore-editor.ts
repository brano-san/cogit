import type { IgnoreHit, IgnorePreview, IgnoreScope } from "$lib/ipc";

export type LineNote = "duplicate" | "unused";

export interface IgnoreTarget {
  scope: IgnoreScope;
  dir: string;
}

export function lineNotes(text: string, target: IgnoreTarget, preview: IgnorePreview | null): Map<number, LineNote> {
  const notes = new Map<number, LineNote>();
  const complete = preview !== null && preview.total === preview.hits.length;
  const used = new Set(
    (preview?.hits ?? []).filter((hit) => hit.scope === target.scope && hit.dir === target.dir).map((hit) => hit.line),
  );
  const seen = new Set<string>();
  text.split("\n").forEach((raw, at) => {
    const line = raw.trim();
    const number = at + 1;
    if (line === "" || line.startsWith("#")) return;
    if (seen.has(line)) notes.set(number, "duplicate");
    else if (complete && !line.startsWith("!") && !used.has(number)) notes.set(number, "unused");
    seen.add(line);
  });
  return notes;
}

export function hitSource(hit: IgnoreHit): string {
  switch (hit.scope) {
    case "folder":
      return `${hit.dir === "" ? "" : `${hit.dir}/`}.gitignore:${hit.line}`;
    case "exclude":
      return `info/exclude:${hit.line}`;
    case "global":
      return `global:${hit.line}`;
  }
}

export function parentDirs(path: string): string[] {
  const parts = path.split("/").slice(0, -1);
  const dirs: string[] = [];
  for (let count = parts.length; count > 0; count--) dirs.push(parts.slice(0, count).join("/"));
  return [...dirs, ""];
}

export interface HitGroup {
  key: string;
  source: string;
  scope: IgnoreScope;
  dir: string;
  line: number;
  pattern: string;
  hits: IgnoreHit[];
}

const SCOPE_ORDER: Record<IgnoreScope, number> = { folder: 0, exclude: 1, global: 2 };

export function groupHits(hits: readonly IgnoreHit[]): HitGroup[] {
  const groups = new Map<string, HitGroup>();
  for (const hit of hits) {
    const key = `${hit.scope}\u0000${hit.dir}\u0000${hit.line}`;
    let group = groups.get(key);
    if (!group) {
      group = { key, source: hitSource(hit), scope: hit.scope, dir: hit.dir, line: hit.line, pattern: hit.pattern, hits: [] };
      groups.set(key, group);
    }
    group.hits.push(hit);
  }
  return [...groups.values()].sort(
    (a, b) => SCOPE_ORDER[a.scope] - SCOPE_ORDER[b.scope] || a.dir.localeCompare(b.dir) || a.line - b.line,
  );
}

export function negation(path: string, target: IgnoreTarget): string | null {
  const base = target.scope === "folder" && target.dir !== "" ? `${target.dir}/` : "";
  if (!path.startsWith(base)) return null;
  return `!/${path.slice(base.length).replace(/[\\*?[]/g, (special) => `\\${special}`)}`;
}

export type LineEdit = "comment" | "remove" | { append: string };

export function editLine(text: string, line: number, edit: LineEdit): string {
  if (typeof edit === "object") {
    const head = text === "" || text.endsWith("\n") ? text : `${text}\n`;
    return `${head}${edit.append}\n`;
  }
  const lines = text.split("\n");
  if (line < 1 || line > lines.length) return text;
  if (edit === "remove") lines.splice(line - 1, 1);
  else lines[line - 1] = `# ${lines[line - 1]}`;
  return lines.join("\n");
}
