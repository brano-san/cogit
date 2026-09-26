/** What Push To sends: a local branch or a tag. */
export interface PushSource {
  kind: "branch" | "tag";
  name: string;
  upstream: string | null;
  /** The remote Push To opens on: the one whose heading in Branches it was asked from. */
  remote?: string;
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
  if (source.remote !== undefined && remotes.includes(source.remote)) return source.remote;
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

/** A branch that tracks nothing yet tracks what it becomes on the remote (R-550). */
export function tracksByDefault(source: PushSource): boolean {
  return source.kind === "branch" && source.upstream === null;
}

/** A branch never pushed, with more than one remote to publish it on: Push opens Push To
    to pick one, as SmartGit's does (R-551). */
export function choosesRemote(source: PushSource, remotes: readonly string[]): boolean {
  return tracksByDefault(source) && remotes.length > 1;
}

/** Push in the menu of a branch or tag: where Push To would send it with no change. */
export function menuPush(
  source: PushSource,
  remotes: readonly string[],
  primary: string | null,
): { remote: string; refspec: string; track: boolean } | null {
  const remote = initialRemote(source, remotes, primary);
  if (remote === null) return null;
  const refspec = pushRefspec(source, { mode: "tracked" }, remote, remotes);
  return { remote, refspec, track: tracksByDefault(source) };
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
