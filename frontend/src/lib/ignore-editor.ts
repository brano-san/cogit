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
