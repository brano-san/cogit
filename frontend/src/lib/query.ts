import type { CommitQuery } from "$lib/ipc";

const EMPTY: CommitQuery = {
  author: null,
  message: null,
  oidPrefix: null,
  since: null,
  until: null,
  path: null,
};

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
  const two = (n: number) => String(n).padStart(2, "0");
  const day = (seconds: number) => {
    const date = new Date(seconds * 1000);
    return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
  };
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

/** Midnight `days` after the start of `value`, a day of the user's own time zone: the
    graph dates a commit in its own zone, not in UTC. Counted in days, not seconds, so a
    day that changes the clock is still one day. */
function localMidnight(value: string, days = 0): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const [year, month, day] = [Number(match[1]), Number(match[2]) - 1, Number(match[3])];
  const date = new Date(year, month, day);
  if (date.getFullYear() !== year || date.getMonth() !== month || date.getDate() !== day) return null;
  return new Date(year, month, day + days).getTime() / 1000;
}

function startOfDay(value: string): number | null {
  return localMidnight(value);
}

function endOfDay(value: string): number | null {
  const next = localMidnight(value, 1);
  return next === null ? null : next - 1;
}
