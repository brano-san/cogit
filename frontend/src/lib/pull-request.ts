export type Forge = "github" | "gitlab" | "bitbucket" | "bitbucket-server" | "azure-devops";

export const FORGES: readonly Forge[] = ["github", "gitlab", "bitbucket", "bitbucket-server", "azure-devops"];

export const HOSTING_KEY = "cogit.hostingProvider";

export interface Remote {
  host: Forge;
  base: string;
}

const HOSTS: Record<string, Forge> = {
  "github.com": "github",
  "gitlab.com": "gitlab",
  "bitbucket.org": "bitbucket",
  "dev.azure.com": "azure-devops",
  "ssh.dev.azure.com": "azure-devops",
  "vs-ssh.visualstudio.com": "azure-devops",
};

const STASH_SSH_PORT = "7999";

interface Location {
  web: "https" | "http";
  ssh: boolean;
  host: string;
  port: string | null;
  segments: string[];
}

function segment(raw: string): string {
  try {
    return encodeURIComponent(decodeURIComponent(raw));
  } catch {
    return encodeURIComponent(raw);
  }
}

function locate(url: string): Location | null {
  const trimmed = url.trim();
  const full = trimmed.match(/^(https?|ssh|git\+ssh|ssh\+git):\/\/(?:[^@/]+@)?([\w.-]+)(?::(\d+))?\/(.+)$/i);
  const scp = full ? null : trimmed.match(/^[^@/\s]+@([\w.-]+):(?!\d+\/)(.+)$/);
  const host = full?.[2] ?? scp?.[1];
  const path = full?.[4] ?? scp?.[2];
  if (!host || !path) return null;
  const scheme = full?.[1]?.toLowerCase() ?? "ssh";
  const segments = path
    .replace(/\/+$/, "")
    .replace(/\.git$/i, "")
    .split("/")
    .filter((part) => part !== "")
    .map(segment);
  if (segments.length === 0) return null;
  return {
    web: scheme === "http" ? "http" : "https",
    ssh: !scheme.startsWith("http"),
    host: host.toLowerCase(),
    port: full?.[3] ?? null,
    segments,
  };
}

function origin(place: Location, host = place.host): string {
  const port = place.ssh || place.port === null ? "" : `:${place.port}`;
  return `${place.web}://${host}${port}`;
}

function isAzure(place: Location): boolean {
  return (
    HOSTS[place.host] === "azure-devops" ||
    place.host.endsWith(".visualstudio.com") ||
    place.segments.slice(0, -1).includes("_git")
  );
}

function isStash(place: Location): boolean {
  if (place.ssh) return place.port === STASH_SSH_PORT && place.segments.length === 2;
  return place.segments.at(-3) === "scm";
}

function detect(place: Location): Forge | null {
  const known = HOSTS[place.host];
  if (known) return known;
  if (isAzure(place)) return "azure-devops";
  if (isStash(place)) return "bitbucket-server";
  return null;
}

function azureWebHost(host: string): string {
  return host === "ssh.dev.azure.com" ? "dev.azure.com" : host;
}

function azureBase(place: Location): string | null {
  const at = place.segments.lastIndexOf("_git");
  const first = place.host.endsWith(".visualstudio.com") ? 0 : 1;
  if (at >= first && at === place.segments.length - 2) {
    return `${origin(place, azureWebHost(place.host))}/${place.segments.join("/")}`;
  }
  const [v3, org, project, repo, ...rest] = place.segments;
  if (v3 !== "v3" || !org || !project || !repo || rest.length > 0) return null;
  const site =
    place.host === "vs-ssh.visualstudio.com"
      ? `https://${org}.visualstudio.com/${project}`
      : `https://${azureWebHost(place.host)}/${org}/${project}`;
  return `${site}/_git/${repo}`;
}

function stashBase(place: Location): string | null {
  const scm = !place.ssh && place.segments.at(-3) === "scm";
  if (!scm && !(place.ssh && place.segments.length === 2)) return null;
  const [project, repo] = place.segments.slice(-2);
  if (!project || !repo) return null;
  const context = scm ? place.segments.slice(0, -3) : [];
  const owner = project.startsWith("~") ? ["users", project.slice(1)] : ["projects", project.toUpperCase()];
  return [origin(place), ...context, ...owner, "repos", repo].join("/");
}

function baseFor(place: Location, host: Forge, detected: boolean): string | null {
  if (host === "azure-devops") return azureBase(place);
  if (host === "bitbucket-server") return stashBase(place);
  const site = detected ? `https://${place.host}` : origin(place);
  return `${site}/${place.segments.join("/")}`;
}

export function forgeOverride(value: string | null | undefined): Forge | null {
  const name = value?.trim().toLowerCase() ?? "";
  return FORGES.find((forge) => forge === name) ?? null;
}

export function forgeSetting(
  settings: readonly { key: string; local: string | null; inherited: string | null }[],
): Forge | null {
  const entry = settings.find((each) => each.key === HOSTING_KEY);
  return forgeOverride(entry ? (entry.local ?? entry.inherited) : null);
}

/** `git@host:path.git`, `ssh://git@host[:port]/path.git` and `https://host/path.git` all
    become a browsable base URL. */
export function parseRemote(url: string, host: Forge | null = null): Remote | null {
  const place = locate(url);
  if (!place) return null;
  const forge = host ?? detect(place);
  if (!forge) return null;
  const base = baseFor(place, forge, host === null);
  return base ? { host: forge, base } : null;
}

/** `base` null leaves the target to the forge, which knows its own default branch. */
export function pullRequestUrl(
  remoteUrl: string,
  base: string | null,
  head: string,
  title: string,
  host: Forge | null = null,
): string | null {
  if (base === head) return null;
  const remote = parseRemote(remoteUrl, host);
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
    case "bitbucket-server": {
      const params = new URLSearchParams({
        sourceBranch: `refs/heads/${head}`,
        ...(base === null ? {} : { targetBranch: `refs/heads/${base}` }),
      });
      return `${remote.base}/pull-requests?create&${params}`;
    }
    case "azure-devops": {
      const params = new URLSearchParams({ sourceRef: head, ...(base === null ? {} : { targetRef: base }) });
      return `${remote.base}/pullrequestcreate?${params}`;
    }
  }
}

export interface PullRequestBranch {
  name: string;
  upstream: string | null;
  ahead: number;
  /** `ahead` against where a push lands, when that is not the upstream. */
  pushAhead?: number | null;
}

export const NO_FORGE = "No GitHub, GitLab, Bitbucket or Azure DevOps remote";

/** Create Pull Request for the checked-out branch, or why it cannot be made. The target
    is the forge's default branch: the branch's own upstream is where it is pushed to, not
    what it is merged into (R-460). The title is the subject of its last commit. */
export function pullRequestFor(input: {
  remoteUrl: string | null;
  branch: PullRequestBranch | undefined;
  title: string | null;
  host?: Forge | null;
}): { url: string } | { reason: string } {
  if (!input.branch) return { reason: "HEAD is not on a branch" };
  const url = input.remoteUrl
    ? pullRequestUrl(input.remoteUrl, null, input.branch.name, input.title ?? input.branch.name, input.host ?? null)
    : null;
  return url ? { url } : { reason: NO_FORGE };
}

/** The form compares what the remote has: a branch never pushed is not there at all. */
export function needsPush(branch: PullRequestBranch): boolean {
  return branch.upstream === null || (branch.pushAhead ?? branch.ahead) > 0;
}

/**
 * The key a token is stored under, as `app_state::token_host` reads it. HTTPS only: SSH has
 * none (the agent authenticates it) and a token is never sent over plain HTTP.
 */
export function authHost(url: string | null | undefined): string | null {
  const match = url?.match(/^https:\/\/(?:[^/]*@)?([\w.-]+)(?::\d+)?(?:\/|$)/);
  return match?.[1] ?? null;
}
