import type { CommitQuery } from "$lib/ipc";

const EMPTY: CommitQuery = {
  author: null,
  message: null,
  oidPrefix: null,
  since: null,
  until: null,
  path: null,
};

const SECONDS_PER_DAY = 86_400;

/** `author:name path:src since:2026-01-01 free words`; anything left over is the message. */
export function parseQuery(input: string): CommitQuery {
  const query: CommitQuery = { ...EMPTY };
  const words: string[] = [];

  for (const token of tokenize(input)) {
    const field = token.match(/^(author|path|oid|since|until):(.*)$/s);
    if (!field) {
      words.push(token);
      continue;
    }
    const [, name, raw] = field;
    const value = unquote(raw ?? "");
    if (value === "") continue;

    switch (name) {
      case "author":
        query.author = value;
        break;
      case "path":
        query.path = value;
        break;
      case "oid":
        query.oidPrefix = value;
        break;
      case "since":
        query.since = startOfDay(value);
        break;
      case "until":
        query.until = endOfDay(value);
        break;
    }
  }

  const message = words.join(" ").trim();
  if (message !== "") query.message = message;
  return query;
}

export function isEmptyQuery(query: CommitQuery): boolean {
  return Object.values(query).every((value) => value === null);
}

/** The text `parseQuery` reads back as `query`: what the filter field shows for a filter
    set elsewhere (File ▸ Log). */
export function formatQuery(query: CommitQuery): string {
  const quote = (value: string) => (/\s/.test(value) ? `"${value}"` : value);
  const day = (seconds: number) => new Date(seconds * 1000).toISOString().slice(0, 10);
  const { author, path, oidPrefix, since, until, message } = query;
  const parts: string[] = [];
  if (author != null) parts.push(`author:${quote(author)}`);
  if (path != null) parts.push(`path:${quote(path)}`);
  if (oidPrefix != null) parts.push(`oid:${quote(oidPrefix)}`);
  if (since != null) parts.push(`since:${day(since)}`);
  if (until != null) parts.push(`until:${day(until)}`);
  if (message != null) parts.push(message);
  return parts.join(" ");
}

/** The same filter: the fields a user types, not how the graph lays the result out. */
export function sameQuery(a: CommitQuery, b: CommitQuery): boolean {
  return (Object.keys(EMPTY) as (keyof CommitQuery)[]).every((key) => (a[key] ?? null) === (b[key] ?? null));
}

/** Splits on whitespace but keeps `field:"two words"` in one piece. */
function tokenize(input: string): string[] {
  return input.match(/(?:[^\s"]|"[^"]*")+/g) ?? [];
}

function unquote(value: string): string {
  return value.startsWith('"') && value.endsWith('"') && value.length >= 2
    ? value.slice(1, -1)
    : value;
}

function startOfDay(value: string): number | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const parsed = Date.parse(`${value}T00:00:00Z`);
  return Number.isNaN(parsed) ? null : parsed / 1000;
}

function endOfDay(value: string): number | null {
  const start = startOfDay(value);
  return start === null ? null : start + SECONDS_PER_DAY - 1;
}
