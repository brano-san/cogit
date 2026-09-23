import type {
  BlameCommit,
  BlameSource,
  BlameTables,
  OriginLine,
  OriginQuery,
} from "$lib/ipc/investigate";

export const UNCOMMITTED = "0".repeat(40);

export function commitOf(tables: BlameTables, line: OriginLine): BlameCommit | undefined {
  const source = tables.sources[line.source];
  return source ? tables.commits[source.commit] : undefined;
}

export function sourceOf(tables: BlameTables, line: OriginLine): BlameSource | undefined {
  return tables.sources[line.source];
}

/** SmartGit Blame's Status column: `+` added, `~` modified, `M` for a merge commit. */
export function markerOf(tables: BlameTables, line: OriginLine): string {
  const merge = commitOf(tables, line)?.merge ? "M" : "";
  return merge + (line.change === "added" ? "+" : "~");
}

/** Compact age, as SmartGit's `21d`: hours under a day, days under a thousand. */
export function ageOf(timestamp: number, now: number): string {
  const seconds = Math.max(0, now - timestamp);
  const hours = Math.floor(seconds / 3600);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(seconds / 86400);
  if (days < 1000) return `${days}d`;
  return `${Math.floor(days / 365)}y`;
}

export function dateOf(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function shortAuthor(name: string): string {
  const first = name.trim().split(/\s+/)[0] ?? "";
  return first.length > 10 ? `${first.slice(0, 9)}…` : first || "?";
}

/** Indices into `lines`, end exclusive. */
export interface Block {
  start: number;
  end: number;
}

/** The lines around `index` one commit wrote as one piece: the same source and
    consecutive line numbers in it. The same maximal block for every line inside it. */
export function blockAt(tables: BlameTables, index: number): Block {
  const lines = tables.lines;
  const joined = (a: OriginLine | undefined, b: OriginLine | undefined) =>
    !!a && !!b && a.source === b.source && b.origLine === a.origLine + 1;
  let start = index;
  while (start > 0 && joined(lines[start - 1], lines[start])) start -= 1;
  let end = index + 1;
  while (end < lines.length && joined(lines[end - 1], lines[end])) end += 1;
  return { start, end };
}

export function originQuery(tables: BlameTables, index: number): OriginQuery | null {
  const picked = tables.lines[index];
  if (!picked) return null;
  const block = blockAt(tables, index);
  const first = tables.lines[block.start];
  const source = first ? tables.sources[first.source] : undefined;
  const commit = source ? tables.commits[source.commit] : undefined;
  if (!first || !source || !commit) return null;
  return {
    commit: commit.oid,
    path: source.path,
    from: first.origLine,
    to: first.origLine + (block.end - block.start) - 1,
    line: picked.origLine,
    previous: source.previous,
  };
}

/** The runs of lines the viewed version itself introduced: its own changes. */
export function changeBlocks(tables: BlameTables, rev: string | null): Block[] {
  const own = (line: OriginLine) => {
    const commit = commitOf(tables, line);
    return rev === null ? !!commit?.uncommitted : commit?.oid === rev;
  };
  const blocks: Block[] = [];
  tables.lines.forEach((line, index) => {
    if (!own(line)) return;
    const last = blocks[blocks.length - 1];
    if (last && last.end === index) last.end = index + 1;
    else blocks.push({ start: index, end: index + 1 });
  });
  return blocks;
}

export function nextChange(blocks: readonly Block[], index: number, direction: 1 | -1): number | null {
  if (direction === 1) return blocks.find((block) => block.start > index)?.start ?? null;
  const before = blocks.filter((block) => block.end <= index);
  return before[before.length - 1]?.start ?? null;
}

const LANGUAGES: Record<string, string> = {
  rs: "rust",
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  svelte: "html",
  py: "python",
  c: "c",
  h: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  hh: "cpp",
  json: "json",
  html: "html",
  htm: "html",
  css: "css",
};

/** The Lezer grammar for a path, among the ones the frontend bundles. */
export function languageOf(path: string): string | null {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  return dot < 0 ? null : (LANGUAGES[name.slice(dot + 1).toLowerCase()] ?? null);
}
