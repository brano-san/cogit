import type { ContextItem } from "./ipc";
import type { RefNode } from "./ref-nodes";
import { SEPARATOR, offer, tidy } from "./context-menu";

/** The Branches menus of rows that are not a ref of their own — HEAD, the headings, the
    folders — and of a remote's heading (#19 of 25.09). */
export const GROUP_MENU_PREFIX = "refgroup:";

const id = (name: string) => `${GROUP_MENU_PREFIX}${name}`;

const NOT_CONFIGURED = "not a configured remote";
const DETACHED = "HEAD is not on a branch";

export function claimsNode(node: RefNode): boolean {
  return node.kind === "head" || node.kind === "group" || node.kind === "folder";
}

/** `toggle`: why the row's box cannot change, or null. */
export function groupMenu(node: RefNode, toggle: string | null): ContextItem[] {
  const own =
    node.id === "group:local"
      ? [offer(id("add-branch"), "Add Branch…", null)]
      : node.id === "group:tags"
        ? [offer(id("add-tag"), "Add Tag…", null)]
        : [];
  return tidy([...own, SEPARATOR, offer(id("toggle"), "Toggle", toggle)]);
}

export interface RemoteFacts {
  remote: string;
  /** False for a heading of remote branches left behind by a remote that is gone. */
  configured: boolean;
  /** HEAD's branch; null when HEAD is detached. */
  head: { name: string; upstream: string | null } | null;
  /** The remote of HEAD's upstream, split by the configured names. */
  upstreamRemote: string | null;
  url: string | null;
  shallow: boolean;
  toggle: string | null;
}

/** Pull from this remote: a pull of the branch that tracks it, a fetch of it when the
    branch tracks nothing (R-552), null when the branch tracks another remote. */
export function remotePull(facts: RemoteFacts): "pull" | "fetch" | null {
  if (facts.head === null) return null;
  if (facts.head.upstream === null) return "fetch";
  return facts.upstreamRemote === facts.remote ? "pull" : null;
}

function pullBlocked(facts: RemoteFacts): string | null {
  if (!facts.configured) return NOT_CONFIGURED;
  if (facts.head === null) return DETACHED;
  return remotePull(facts) === null ? `${facts.head.name} tracks ${facts.head.upstream}` : null;
}

export function remoteMenu(facts: RemoteFacts): ContextItem[] {
  const gone = facts.configured ? null : NOT_CONFIGURED;
  return tidy([
    offer(id("push-to"), "Push To…", gone ?? (facts.head === null ? DETACHED : null)),
    SEPARATOR,
    offer(id("pull"), "Pull", pullBlocked(facts)),
    offer(id("fetch"), "Fetch", gone),
    offer(id("fetch-more"), "Fetch More", gone),
    SEPARATOR,
    offer(id("rename"), "Rename…", gone),
    offer(id("delete"), "Delete", gone),
    SEPARATOR,
    offer(id("copy-url"), "Copy URL", facts.url ? null : "no URL"),
    SEPARATOR,
    offer(id("set-depth"), "Set Depth…", gone ?? (facts.shallow ? null : "not a shallow clone")),
    offer(id("properties"), "Properties…", gone),
    SEPARATOR,
    offer(id("toggle"), "Toggle", facts.toggle),
  ]);
}

/** The cheap half of git's rules for a remote name, which must make `refs/remotes/<name>/…`
    a valid ref; git has the last word. */
export function remoteNameProblem(value: string, taken: readonly string[], current: string): string | null {
  const name = value.trim();
  if (name === "") return "Enter a name for the remote.";
  if (/[\s~^:?*[\\]|\.\.|@\{|\/\//.test(name) || /^[-/.]|[/.]$|\.lock$/.test(name)) {
    return "Git will refuse that name.";
  }
  if (name !== current && taken.includes(name)) return `There is a remote ${name} already.`;
  return null;
}

const MAX_DEPTH = 2 ** 31 - 1;

export function depthProblem(value: string): string | null {
  const text = value.trim();
  const depth = Number(text);
  if (!/^\d+$/.test(text) || depth < 1 || depth > MAX_DEPTH) return "Enter a number of commits, 1 or more.";
  return null;
}
