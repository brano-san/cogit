import { Channel } from "@tauri-apps/api/core";

import { commands } from "./bindings";
import type { CommitQuery, DiffOptions, DiffSpec, GitError, GraphChunk, RepoId } from "./bindings";

export type {
  AppInfo,
  Branch,
  BranchKind,
  Algorithm,
  CommitDetails,
  CommitQuery,
  CommitRow,
  DiffOptions,
  DiffRow,
  DiffSpec,
  EolInfo,
  FileDiff,
  FileEntry,
  FileStatus,
  GitError,
  GraphChunk,
  GraphEdge,
  Head,
  Hunk,
  LaneAssignment,
  LineEnding,
  RepoId,
  RepoStatus,
  RepoSummary,
  Signature,
  Tag,
  Whitespace,
} from "./bindings";

/** Carries the raw `GitError` untouched so the dialog can show Git's own output (INV-05). */
export class CogitError extends Error {
  readonly detail: GitError;

  constructor(detail: GitError) {
    super(describeError(detail));
    this.name = "CogitError";
    this.detail = detail;
  }

  get isCommandFailure(): boolean {
    return this.detail.kind === "command";
  }
}

function describeError(error: GitError): string {
  switch (error.kind) {
    case "command":
      return `${error.data.command} failed with exit code ${error.data.exitCode ?? "unknown"}`;
    case "repoNotFound":
      return `Not a Git repository: ${error.data}`;
    case "repoBusy":
      return `Repository is busy: ${error.data}`;
    case "invalidState":
      return `Invalid repository state: ${error.data}`;
    case "io":
      return `I/O error: ${error.data}`;
    case "internal":
      return `Internal error: ${error.data}`;
  }
}

export async function getAppInfo() {
  return await commands.appInfo();
}

export async function openRepository(path: string) {
  const result = await commands.openRepository(path);
  if (result.status === "error") {
    throw new CogitError(result.error);
  }
  return result.data;
}

export const EMPTY_QUERY: CommitQuery = {
  author: null,
  message: null,
  oidPrefix: null,
  since: null,
  until: null,
  path: null,
};

export async function loadCommits(
  repo: RepoId,
  onChunk: (chunk: GraphChunk) => void,
  query: CommitQuery = EMPTY_QUERY,
) {
  const channel = new Channel<GraphChunk>();
  channel.onmessage = onChunk;
  const result = await commands.loadCommits(repo, query, channel);
  if (result.status === "error") {
    throw new CogitError(result.error);
  }
}

export async function commitDetails(repo: RepoId, rev: string) {
  return unwrap(await commands.commitDetails(repo, rev));
}

export async function commitFiles(repo: RepoId, rev: string) {
  return unwrap(await commands.commitFiles(repo, rev));
}

export const DEFAULT_DIFF_OPTIONS: DiffOptions = {
  algorithm: "histogram",
  contextLines: 3,
  ignoreWhitespace: "none",
  ignoreBlankLines: false,
  wordDiff: true,
};

export async function diffFile(
  repo: RepoId,
  spec: DiffSpec,
  path: string,
  options: DiffOptions = DEFAULT_DIFF_OPTIONS,
) {
  return unwrap(await commands.diffFile(repo, spec, path, options));
}

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: GitError }): T {
  if (result.status === "error") {
    throw new CogitError(result.error);
  }
  return result.data;
}
