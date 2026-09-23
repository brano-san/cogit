/** What Push To sends: a local branch or a tag. */
export interface PushSource {
  kind: "branch" | "tag";
  name: string;
  upstream: string | null;
}

export type PushTarget = { mode: "tracked" } | { mode: "custom"; ref: string };

/** `origin/feature/x` split at the longest remote name that prefixes it: remote names may
    contain a slash too. */
export function splitUpstream(
  upstream: string,
  remotes: readonly string[],
): { remote: string; branch: string } | null {
  const owner = [...remotes]
    .sort((a, b) => b.length - a.length)
    .find((remote) => upstream.startsWith(`${remote}/`));
  return owner ? { remote: owner, branch: upstream.slice(owner.length + 1) } : null;
}

export function initialRemote(
  source: PushSource,
  remotes: readonly string[],
  primary: string | null,
): string | null {
  const tracked = source.upstream ? splitUpstream(source.upstream, remotes) : null;
  return tracked?.remote ?? primary ?? remotes[0] ?? null;
}

function qualify(ref: string, namespace: string): string {
  const trimmed = ref.trim();
  return trimmed.startsWith("refs/") ? trimmed : `${namespace}${trimmed}`;
}

export function targetRef(
  source: PushSource,
  target: PushTarget,
  remote: string,
  remotes: readonly string[],
): string {
  const namespace = source.kind === "tag" ? "refs/tags/" : "refs/heads/";
  if (target.mode === "custom") return qualify(target.ref, namespace);
  if (source.kind === "tag") return `refs/tags/${source.name}`;
  const tracked = source.upstream ? splitUpstream(source.upstream, remotes) : null;
  return tracked?.remote === remote
    ? `refs/heads/${tracked.branch}`
    : `refs/heads/${source.name}`;
}

export function pushRefspec(
  source: PushSource,
  target: PushTarget,
  remote: string,
  remotes: readonly string[],
): string {
  const from = source.kind === "tag" ? `refs/tags/${source.name}` : `refs/heads/${source.name}`;
  return `${from}:${targetRef(source, target, remote, remotes)}`;
}

/** Push Up To: every commit of HEAD's branch up to `oid`, onto its upstream. */
export function pushUpTo(
  oid: string,
  upstream: string,
  remotes: readonly string[],
): { remote: string; refspec: string } | null {
  const tracked = splitUpstream(upstream, remotes);
  return tracked ? { remote: tracked.remote, refspec: `${oid}:refs/heads/${tracked.branch}` } : null;
}

/** The cheap half of Git's ref-name rules; the server has the last word. */
export function customRefProblem(ref: string): string | null {
  const trimmed = ref.trim();
  if (trimmed === "") return "Enter the name of the ref to push to.";
  if (/[\s~^:?*[\\]|\.\.|@\{|\/\//.test(trimmed)) return "Git will refuse that name.";
  if (/^[-/]|\/$|\.lock$|\.$/.test(trimmed)) return "Git will refuse that name.";
  return null;
}

export function pushTitle(source: PushSource, remote: string): string {
  return `Push '${source.name}' to remote '${remote}'`;
}
