import { Channel } from "@tauri-apps/api/core";

import { commands, events } from "./bindings";
import type {
  CheckoutTarget,
  CommitQuery,
  CommitRequest,
  DiffOptions,
  DiffSpec,
  GitError,
  GraphChunk,
  RepoChanged,
  RepoId,
} from "./bindings";

export type {
  Algorithm,
  AppInfo,
  Branch,
  BranchKind,
  CheckoutTarget,
  CommitDetails,
  CommitQuery,
  CommitRequest,
  CommitRow,
  DiffOptions,
  DiffRow,
  DiffSpec,
  EolInfo,
  FileDiff,
  FileEntry,
  FileStatus,
  GitError,
  GitOutput,
  GraphChunk,
  GraphEdge,
  Head,
  Hunk,
  LaneAssignment,
  LineEnding,
  RepoChanged,
  RepoId,
  RepoStatus,
  RepoSummary,
  SafetyEntry,
  Signature,
  Tag,
  Whitespace,
  WorktreeFiles,
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

export async function repoStatus(repo: RepoId) {
  return unwrap(await commands.repoStatus(repo));
}

export async function worktreeFiles(repo: RepoId) {
  return unwrap(await commands.worktreeFiles(repo));
}

export async function stagePaths(repo: RepoId, paths: string[]) {
  return unwrap(await commands.stagePaths(repo, paths));
}

export async function unstagePaths(repo: RepoId, paths: string[]) {
  return unwrap(await commands.unstagePaths(repo, paths));
}

export async function discardPaths(repo: RepoId, paths: string[]) {
  return unwrap(await commands.discardPaths(repo, paths));
}

export async function createCommit(repo: RepoId, request: CommitRequest) {
  return unwrap(await commands.commit(repo, request));
}

export async function checkout(repo: RepoId, target: CheckoutTarget) {
  return unwrap(await commands.checkout(repo, target));
}

export async function createBranch(
  repo: RepoId,
  name: string,
  start: string | null,
  switchTo: boolean,
) {
  return unwrap(await commands.createBranch(repo, name, start, switchTo));
}

export async function deleteBranch(repo: RepoId, name: string, force: boolean) {
  return unwrap(await commands.deleteBranch(repo, name, force));
}

/** Fires when the watcher sees the repository change on disk; returns an unlisten fn. */
export async function onRepoChanged(handler: (change: RepoChanged) => void) {
  return await events.repoChanged.listen((event) => handler(event.payload));
}

export async function commandLog() {
  return await commands.commandLog();
}

export async function safetyLog() {
  return await commands.safetyLog();
}

export async function undoLast(repo: RepoId) {
  return unwrap(await commands.undoLast(repo));
}

export async function commandProblems() {
  return await commands.commandProblems();
}

export async function clearCommandLog() {
  await commands.clearCommandLog();
}

function unwrap<T>(result: { status: "ok"; data: T } | { status: "error"; error: GitError }): T {
  if (result.status === "error") {
    throw new CogitError(result.error);
  }
  return result.data;
}
