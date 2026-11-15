import type { Head } from "./ipc/bindings";

/** A commit message prefix taken from the branch name (F-943): a regular expression over
    the branch and a template with `$1`…`$9`. Preferences hold one rule for every repository;
    Repository ▸ Settings may override it with `cogit.commitPrefixPattern` / `…Template`. */
export interface PrefixRule {
  pattern: string;
  template: string;
}

export const PATTERN_KEY = "cogit.commitPrefixPattern";
export const TEMPLATE_KEY = "cogit.commitPrefixTemplate";

/** What is wrong with the pattern, for the field to show; null for a valid or empty one. */
export function patternProblem(pattern: string): string | null {
  if (pattern.trim() === "") return null;
  try {
    new RegExp(pattern);
    return null;
  } catch (err) {
    const why = err instanceof Error ? err.message.replace(/^Invalid regular expression: /, "") : String(err);
    return `Not a valid regular expression: ${why}`;
  }
}

/** `$1`…`$9` become the groups (an unmatched one is empty), `$&` the whole match, `$$` a
    dollar; anything else stays as written. */
export function expandTemplate(template: string, match: RegExpExecArray): string {
  return template.replace(/\$(\$|&|[1-9])/g, (_, what: string) => {
    if (what === "$") return "$";
    if (what === "&") return match[0];
    return match[Number(what)] ?? "";
  });
}

/** The prefix for `branch`, or null: no branch, no rule, an invalid pattern, no match, or
    a template that comes out empty. */
export function branchPrefix(rule: PrefixRule, branch: string | null): string | null {
  if (branch === null || rule.pattern.trim() === "" || patternProblem(rule.pattern) !== null) return null;
  const match = new RegExp(rule.pattern).exec(branch);
  if (match === null) return null;
  const prefix = expandTemplate(rule.template, match);
  return prefix === "" ? null : prefix;
}

/** The branch HEAD is on, born or not; null when detached or unknown. */
export function headBranch(head: Head | null | undefined): string | null {
  if (!head || head.kind === "detached") return null;
  return head.name.replace(/^refs\/heads\//, "");
}

/** The repository's own value of a key wins over Preferences when it is set at all; set
    to an empty pattern it turns the prefix off there. */
export function effectiveRule(
  global: PrefixRule,
  repo: { pattern: string | null; template: string | null },
): PrefixRule {
  if (repo.pattern === null) return global;
  return { pattern: repo.pattern, template: repo.template ?? global.template };
}

/** The repository's values from Repository ▸ Settings (`repo_settings`): set in this
    repository or in the user's git config. */
export function repoRule(
  settings: readonly { key: string; local: string | null; inherited: string | null }[],
): { pattern: string | null; template: string | null } {
  const value = (key: string) => {
    const entry = settings.find((each) => each.key === key);
    return entry ? (entry.local ?? entry.inherited) : null;
  };
  return { pattern: value(PATTERN_KEY), template: value(TEMPLATE_KEY) };
}

/** What an empty field opens with: what git prepared (`SQUASH_MSG`, `MERGE_MSG`) or
    `commit.template` comes first and is never mixed with a prefix. */
export function messageSeed(template: string | null, prefix: string | null): string | null {
  return template ?? prefix;
}

/** The message Amend brings back, prefixed unless it already starts with the prefix. */
export function amendedMessage(last: string, prefix: string | null): string {
  if (prefix === null || last.startsWith(prefix) || last.startsWith(prefix.trimEnd())) return last;
  return prefix + last;
}
