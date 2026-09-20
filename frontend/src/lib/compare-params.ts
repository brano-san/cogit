import type { DiffSpec, RepoId } from "./ipc";

export interface CompareRequest {
  repo: RepoId;
  path: string;
  spec: DiffSpec;
}

/** In the URL, not in shared state: the window has to survive a webview reload (T2.5). */
export function compareUrl(repo: RepoId, path: string, spec: DiffSpec): string {
  const params = new URLSearchParams({ repo: String(repo), path, kind: spec.kind });
  if (spec.kind === "commitVsParent") {
    params.set("oid", spec.oid);
  }
  if (spec.kind === "commitVsCommit") {
    params.set("a", spec.a);
    params.set("b", spec.b);
  }
  return `compare.html?${params.toString()}`;
}

export function parseCompare(search: string): CompareRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const path = params.get("path");
  const kind = params.get("kind");
  if (!Number.isInteger(repo) || !path || !kind) return null;

  const spec = specOf(kind, params);
  return spec === null ? null : { repo, path, spec };
}

function specOf(kind: string, params: URLSearchParams): DiffSpec | null {
  switch (kind) {
    case "workTreeVsIndex":
    case "indexVsHead":
      return { kind };
    case "commitVsParent": {
      const oid = params.get("oid");
      return oid ? { kind, oid } : null;
    }
    case "commitVsCommit": {
      const a = params.get("a");
      const b = params.get("b");
      return a && b ? { kind, a, b } : null;
    }
    default:
      return null;
  }
}
