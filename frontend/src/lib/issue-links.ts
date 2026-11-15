/** Issue ids in commit messages as links (F-953), the way TortoiseGit reads `bugtraq.*`:
    `logregex` is one regular expression whose groups are the ids (the whole match when it
    has none), or two on separate lines — the first finds the mention, the second the ids
    inside it. Without a regex, `bugtraq.message` (`Issue: %BUGID%`) says where ids stand. */

/** Repository ▸ Settings ▸ Commit: Cogit's own rule, over `bugtraq.*` and `.tgitconfig`. */
export const ISSUE_PATTERN_KEY = "cogit.issuePattern";
export const ISSUE_URL_KEY = "cogit.issueUrl";

export interface IssueTrackerConfig {
  origin: string;
  url: string | null;
  logRegex: string | null;
  message: string | null;
}

export interface IssueRule {
  url: string;
  find: RegExp;
  /** The second regex of the two-line form, run inside each match of `find`. */
  ids: RegExp | null;
}

export interface IssuePart {
  text: string;
  /** The issue's id, when this part is one. */
  issue: string | null;
  href: string | null;
}

const ID_LIST = String.raw`[A-Za-z0-9_-]+(?:\s*,\s*[A-Za-z0-9_-]+)*`;
const ID = String.raw`[A-Za-z0-9_-]+`;

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function compile(source: string): RegExp | null {
  try {
    return new RegExp(source, "gd");
  } catch {
    return null;
  }
}

/** Only a web page is opened: the rule may come from a file in a cloned repository. */
function webUrl(url: string | null): url is string {
  return url !== null && /^https?:\/\/\S+$/i.test(url.trim());
}

function ruleOf(tracker: IssueTrackerConfig): IssueRule | null {
  if (!webUrl(tracker.url)) return null;
  const url = tracker.url.trim();
  const lines = (tracker.logRegex ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== "");
  if (lines.length > 0) {
    const find = compile(lines[0]!);
    const ids = lines.length > 1 ? compile(lines[1]!) : null;
    if (!find || (lines.length > 1 && !ids)) return null;
    return { url, find, ids };
  }
  const message = tracker.message ?? "";
  const at = message.indexOf("%BUGID%");
  if (at < 0) return null;
  const before = message.slice(0, at).trim();
  const after = message.slice(at + "%BUGID%".length).trim();
  const lead = before === "" ? "" : `(?<=${escape(before)}\\s*)`;
  const tail = after === "" ? "" : `(?=\\s*${escape(after)})`;
  const find = compile(`${lead}${ID_LIST}${tail}`);
  const ids = compile(ID);
  return find && ids ? { url, find, ids } : null;
}

/** The first place that gives a usable rule wins (the list comes most specific first). */
export function issueRule(trackers: readonly IssueTrackerConfig[]): IssueRule | null {
  for (const tracker of trackers) {
    const rule = ruleOf(tracker);
    if (rule) return rule;
  }
  return null;
}

/** What Repository ▸ Settings says under an issue URL it cannot use; null when it can. */
export function issueUrlProblem(value: string | null): string | null {
  if (value === null || value.trim() === "") return null;
  if (!webUrl(value)) return "Use an http:// or https:// address";
  if (!value.includes("%BUGID%")) return "Put %BUGID% where the issue's id goes";
  return null;
}

export function issueUrl(template: string, id: string): string {
  return template.replaceAll("%BUGID%", encodeURIComponent(id));
}

interface Found {
  start: number;
  id: string;
}

function* matches(pattern: RegExp, text: string): Generator<RegExpExecArray> {
  pattern.lastIndex = 0;
  for (const match of text.matchAll(pattern)) {
    if (match[0] !== "") yield match;
  }
}

/** The ids of one match of a regex: its groups, or all of it when it has none. */
function idsOf(match: RegExpExecArray, offset: number): Found[] {
  const found: Found[] = [];
  for (let group = 1; group < match.length; group++) {
    const id = match[group];
    const span = match.indices?.[group];
    if (id && span) found.push({ start: offset + span[0], id });
  }
  if (match.length === 1) found.push({ start: offset + match.index, id: match[0] });
  return found;
}

export function findIssues(text: string, rule: IssueRule): Found[] {
  const found: Found[] = [];
  for (const mention of matches(rule.find, text)) {
    if (!rule.ids) {
      found.push(...idsOf(mention, 0));
      continue;
    }
    for (const id of matches(rule.ids, mention[0])) found.push(...idsOf(id, mention.index));
  }
  found.sort((a, b) => a.start - b.start);
  // Overlapping groups (a group inside a group) would split one id in two.
  const kept: Found[] = [];
  for (const each of found) {
    const last = kept.at(-1);
    if (!last || each.start >= last.start + last.id.length) kept.push(each);
  }
  return kept;
}

/** `text` cut into plain parts and issue links; one plain part without a rule. */
export function issueLinks(text: string, rule: IssueRule | null): IssuePart[] {
  if (!rule || text === "") return [{ text, issue: null, href: null }];
  const parts: IssuePart[] = [];
  let at = 0;
  for (const { start, id } of findIssues(text, rule)) {
    if (start > at) parts.push({ text: text.slice(at, start), issue: null, href: null });
    parts.push({ text: id, issue: id, href: issueUrl(rule.url, id) });
    at = start + id.length;
  }
  if (at < text.length) parts.push({ text: text.slice(at), issue: null, href: null });
  return parts;
}
