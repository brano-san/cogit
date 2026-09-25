export type Forge = "github" | "gitlab" | "bitbucket";

export interface Remote {
  host: Forge;
  base: string;
}

const HOSTS: Record<string, Forge> = {
  "github.com": "github",
  "gitlab.com": "gitlab",
  "bitbucket.org": "bitbucket",
};

/** `git@host:path.git`, `ssh://git@host[:port]/path.git` and `https://host/path.git` all
    become a browsable base URL. */
export function parseRemote(url: string): Remote | null {
  const ssh = url.match(/^[\w.-]+@([\w.-]+):(?!\d+\/)(.+?)(?:\.git)?$/);
  const sshUrl = url.match(/^ssh:\/\/(?:[^@/]+@)?([\w.-]+)(?::\d+)?\/(.+?)(?:\.git)?$/);
  const https = url.match(/^https?:\/\/(?:[^@/]+@)?([\w.-]+)\/(.+?)(?:\.git)?$/);
  const match = ssh ?? sshUrl ?? https;
  if (!match) return null;

  const host = HOSTS[match[1]?.toLowerCase() ?? ""];
  if (!host) return null;
  return { host, base: `https://${match[1]}/${match[2]}` };
}

/** `base` null leaves the target to the forge, which knows its own default branch. */
export function pullRequestUrl(
  remoteUrl: string,
  base: string | null,
  head: string,
  title: string,
): string | null {
  if (base === head) return null;
  const remote = parseRemote(remoteUrl);
  if (!remote) return null;

  switch (remote.host) {
    case "github": {
      const params = new URLSearchParams({ expand: "1", title });
      const range = base === null ? encodeURIComponent(head) : `${encodeURIComponent(base)}...${encodeURIComponent(head)}`;
      return `${remote.base}/compare/${range}?${params}`;
    }
    case "gitlab": {
      const params = new URLSearchParams({
        "merge_request[source_branch]": head,
        ...(base === null ? {} : { "merge_request[target_branch]": base }),
        "merge_request[title]": title,
      });
      return `${remote.base}/-/merge_requests/new?${params}`;
    }
    case "bitbucket": {
      const params = new URLSearchParams({ source: head, ...(base === null ? {} : { dest: base }), title });
      return `${remote.base}/pull-requests/new?${params}`;
    }
  }
}

export interface PullRequestBranch {
  name: string;
  upstream: string | null;
  ahead: number;
}

export const NO_FORGE = "No GitHub, GitLab or Bitbucket remote";

/** Create Pull Request for the checked-out branch, or why it cannot be made. The target
    is the forge's default branch: the branch's own upstream is where it is pushed to, not
    what it is merged into (R-460). The title is the subject of its last commit. */
export function pullRequestFor(input: {
  remoteUrl: string | null;
  branch: PullRequestBranch | undefined;
  title: string | null;
}): { url: string } | { reason: string } {
  if (!input.branch) return { reason: "HEAD is not on a branch" };
  const url = input.remoteUrl
    ? pullRequestUrl(input.remoteUrl, null, input.branch.name, input.title ?? input.branch.name)
    : null;
  return url ? { url } : { reason: NO_FORGE };
}

/** The form compares what the remote has: a branch never pushed is not there at all. */
export function needsPush(branch: PullRequestBranch): boolean {
  return branch.upstream === null || branch.ahead > 0;
}

/** The key a token is stored under. SSH has none: the agent already authenticates it. */
export function authHost(url: string | null | undefined): string | null {
  const match = url?.match(/^https?:\/\/(?:[^@/]+@)?([\w.-]+)(?::\d+)?(?:\/|$)/);
  return match?.[1] ?? null;
}
