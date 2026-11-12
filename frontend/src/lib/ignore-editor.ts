import { SEPARATOR, item, offer, tidy } from "$lib/context-menu";
import type { ContextItem, IgnoreHit, IgnorePreview, IgnoreScope } from "$lib/ipc";

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

/** The pattern a line holds, spelled as the preview reports it (gix-ignore's parser), or
    `null` for a comment or a blank line. */
export function ruleText(raw: string): string | null {
  if (raw === "" || raw.startsWith("#")) return null;
  let spaces: number | null = null;
  for (let at = 0; at < raw.length; at++) {
    if (raw[at] === " ") {
      spaces ??= at;
      continue;
    }
    if (raw[at] === "\\") {
      if (at + 1 >= raw.length) {
        spaces = null;
        break;
      }
      at++;
    }
    spaces = null;
  }
  let line = spaces === null ? raw : raw.slice(0, spaces);
  if (line.startsWith("\\!") || line.startsWith("\\#")) line = line.slice(1);
  return line.trim() === "" ? null : line;
}

/** Whether `line` still holds `pattern`: the preview it came from may be older than the text. */
export function ruleAt(text: string, line: number, pattern: string): boolean {
  const raw = text.split("\n")[line - 1];
  return raw !== undefined && ruleText(raw) === pattern;
}

export function nameOf(path: string): { name: string; folder: string } {
  const bare = path.endsWith("/") ? path.slice(0, -1) : path;
  const at = bare.lastIndexOf("/");
  return { name: bare.slice(at + 1) + (bare === path ? "" : "/"), folder: at < 0 ? "" : bare.slice(0, at) };
}

/** `text` is the rule's file as edited, `null` when the dialog does not hold that file;
    `fresh` is false while the preview lags behind the text. */
export function ignoreMenu(group: HitGroup, hit: IgnoreHit | null, text: string | null, fresh: boolean): ContextItem[] {
  const elsewhere = text === null ? "in another .gitignore" : null;
  const rule =
    elsewhere ??
    (!fresh ? "list is updating" : !ruleAt(text ?? "", group.line, group.pattern) ? "line has changed" : null);
  const keep =
    hit === null
      ? null
      : hit.underFolder !== null
        ? "its folder is ignored"
        : (elsewhere ?? (negation(hit.path, group) === null ? "outside this folder" : null));
  return tidy([
    ...(hit ? [offer("ignore-keep", `Don't Ignore ${nameOf(hit.path).name}`, keep), SEPARATOR] : []),
    offer("ignore-go", "Go to Rule", rule),
    offer("ignore-comment", "Comment Out Rule", rule),
    offer("ignore-remove", "Remove Rule", rule),
    ...(hit ? [SEPARATOR, item("ignore-copy", "Copy Path")] : []),
  ]);
}
