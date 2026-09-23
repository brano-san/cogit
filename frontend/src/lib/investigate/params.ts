import type { RepoId } from "$lib/ipc";

/** A file at a commit (`rev: null` is the working tree), optionally at a 1-based line. */
export interface Location {
  path: string;
  rev: string | null;
  line: number | null;
}

export interface InvestigateRequest {
  repo: RepoId;
  repoName: string;
  start: Location;
}

/** In the URL, not in shared state: the window has to survive a webview reload (T2.5). */
export function investigateUrl(request: InvestigateRequest): string {
  const params = new URLSearchParams({
    repo: String(request.repo),
    name: request.repoName,
    path: request.start.path,
  });
  if (request.start.rev) params.set("rev", request.start.rev);
  if (request.start.line) params.set("line", String(request.start.line));
  return `investigate.html?${params.toString()}`;
}

export function parseInvestigate(search: string): InvestigateRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const path = params.get("path");
  if (!Number.isInteger(repo) || repo <= 0 || !path) return null;
  const line = Number(params.get("line"));
  return {
    repo,
    repoName: params.get("name") ?? "",
    start: {
      path,
      rev: params.get("rev") || null,
      line: Number.isInteger(line) && line > 0 ? line : null,
    },
  };
}

export function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

export function investigateTitle(path: string, repoName: string): string {
  const repo = repoName ? ` [${repoName}]` : "";
  return `${fileName(path)}${repo} - Investigate`;
}

export function sameLocation(a: Location, b: Location): boolean {
  return a.path === b.path && a.rev === b.rev && a.line === b.line;
}
