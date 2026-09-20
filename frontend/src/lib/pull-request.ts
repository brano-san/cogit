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

/** `git@host:path.git` and `https://host/path.git` both become a browsable base URL. */
export function parseRemote(url: string): Remote | null {
  const ssh = url.match(/^[\w.-]+@([\w.-]+):(.+?)(?:\.git)?$/);
  const https = url.match(/^https?:\/\/(?:[^@/]+@)?([\w.-]+)\/(.+?)(?:\.git)?$/);
  const match = ssh ?? https;
  if (!match) return null;

  const host = HOSTS[match[1]?.toLowerCase() ?? ""];
  if (!host) return null;
  return { host, base: `https://${match[1]}/${match[2]}` };
}

export function pullRequestUrl(
  remoteUrl: string,
  base: string,
  head: string,
  title: string,
): string | null {
  if (base === head) return null;
  const remote = parseRemote(remoteUrl);
  if (!remote) return null;

  switch (remote.host) {
    case "github": {
      const params = new URLSearchParams({ expand: "1", title });
      return `${remote.base}/compare/${encodeURIComponent(base)}...${encodeURIComponent(head)}?${params}`;
    }
    case "gitlab": {
      const params = new URLSearchParams({
        "merge_request[source_branch]": head,
        "merge_request[target_branch]": base,
        "merge_request[title]": title,
      });
      return `${remote.base}/-/merge_requests/new?${params}`;
    }
    case "bitbucket": {
      const params = new URLSearchParams({ source: head, dest: base, title });
      return `${remote.base}/pull-requests/new?${params}`;
    }
  }
}
