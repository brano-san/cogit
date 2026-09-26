import { shortOid } from "./format";
import type { DiffSpec, RepoId } from "./ipc";

export interface CompareRequest {
  repo: RepoId;
  path: string;
  spec: DiffSpec;
}

export interface CompareLabel {
  text: string;
  /** Which version is on which side. */
  tip: string;
}

/** The two sides in the title and the header of the compare window. `parent` is the first
    parent of a `commitVsParent` commit: `undefined` until it is read, `null` for a root. */
export function compareLabel(spec: DiffSpec, parent?: string | null): CompareLabel {
  switch (spec.kind) {
    case "workTreeVsIndex":
      return {
        text: "Working tree vs index",
        tip: "Changes not staged yet: the index on the left, the file on disk on the right",
      };
    case "indexVsHead":
      return { text: "Index vs HEAD", tip: "Staged changes: HEAD on the left, the index on the right" };
    case "commitVsParent": {
      const commit = shortOid(spec.oid);
      if (parent === null) {
        return {
          text: `Commit ${commit}, the first commit`,
          tip: `What commit ${commit} added: it has no parent, so there is nothing on the left`,
        };
      }
      const named = parent === undefined ? "its first parent" : `its first parent ${shortOid(parent)}`;
      return {
        text: parent === undefined ? `Commit ${commit} vs its parent` : `Commit ${commit} vs parent ${shortOid(parent)}`,
        tip: `What commit ${commit} changed: ${named} on the left, the commit on the right`,
      };
    }
    case "commitVsWorkTree":
      return {
        text: `Commit ${shortOid(spec.oid)} vs working tree`,
        tip: `The file as commit ${shortOid(spec.oid)} has it on the left, the file on disk now on the right`,
      };
    case "commitVsCommit":
      return {
        text: `Commit ${shortOid(spec.a)} vs commit ${shortOid(spec.b)}`,
        tip: `Commit ${shortOid(spec.a)} on the left, commit ${shortOid(spec.b)} on the right`,
      };
  }
}

/** In the URL, not in shared state: the window has to survive a webview reload (T2.5). */
export function compareUrl(repo: RepoId, path: string, spec: DiffSpec): string {
  const params = new URLSearchParams({ repo: String(repo), path, kind: spec.kind });
  if (spec.kind === "commitVsParent" || spec.kind === "commitVsWorkTree") {
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
    case "commitVsParent":
    case "commitVsWorkTree": {
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
