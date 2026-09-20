import { Channel } from "@tauri-apps/api/core";

import { commands, events } from "./bindings";
import type {
  CheckoutTarget,
  CommitQuery,
  CommitRequest,
  ConflictSide,
  DiffOptions,
  DiffSpec,
  GitError,
  GraphChunk,
  MergeOptions,
  PatchRequest,
  RebaseOptions,
  RepoChanged,
  RepoId,
  StashOptions,
  TagRequest,
} from "./bindings";

export type {
  Algorithm,
  AppInfo,
  BlameLine,
  Branch,
  BranchKind,
  CheckoutTarget,
  CommitDetails,
  CommitQuery,
  CommitRequest,
  CommitRow,
  ConflictSide,
  DiffOptions,
  DiffRow,
  DiffSpec,
  EolInfo,
  FileDiff,
  FileEntry,
  FileStatus,
  Found,
  FoundKind,
  GitError,
  GitOutput,
  GraphChunk,
  GraphEdge,
  Head,
  Hunk,
  LaneAssignment,
  LineEnding,
  MergeOptions,
  PatchRequest,
  RebaseOptions,
  ReflogEntry,
  RepoChanged,
  RepoId,
  RepoOverview,
  RepoState,
  RepoStatus,
  RepoSummary,
  SafetyEntry,
  Signature,
  StashEntry,
  StashOptions,
  Submodule,
  SubmoduleState,
  Tag,
  TagRequest,
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
  detectMoves: true,
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

export async function addToGitignore(repo: RepoId, paths: string[]) {
  return unwrap(await commands.addToGitignore(repo, paths));
}

export async function deleteUntracked(repo: RepoId, paths: string[]) {
  return unwrap(await commands.deleteUntracked(repo, paths));
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

export async function findObject(repo: RepoId, query: string, limit = 25) {
  return unwrap(await commands.findObject(repo, query, limit));
}

export async function conflictedPaths(repo: RepoId) {
  return unwrap(await commands.conflictedPaths(repo));
}

export async function conflictText(repo: RepoId, path: string) {
  return unwrap(await commands.conflictText(repo, path));
}

export async function resolveConflict(repo: RepoId, path: string, side: ConflictSide) {
  return unwrap(await commands.resolveConflict(repo, path, side));
}

export async function resolveConflictText(repo: RepoId, path: string, text: string) {
  return unwrap(await commands.resolveConflictText(repo, path, text));
}

export async function imageSides(repo: RepoId, spec: DiffSpec, path: string) {
  return unwrap(await commands.imageSides(repo, spec, path));
}

export async function blameFile(repo: RepoId, path: string, rev: string) {
  return unwrap(await commands.blame(repo, path, rev));
}

export async function stageSelection(repo: RepoId, request: PatchRequest, reverse: boolean) {
  return unwrap(await commands.stageSelection(repo, request, reverse));
}

export async function listSubmodules(repo: RepoId) {
  return unwrap(await commands.submodules(repo));
}

export async function updateSubmodule(repo: RepoId, path: string, init: boolean) {
  return unwrap(await commands.updateSubmodule(repo, path, init));
}

export async function listRepositories() {
  return await commands.repositories();
}

export async function closeRepository(repo: RepoId) {
  return await commands.closeRepository(repo);
}

export async function readReflog(repo: RepoId, limit = 100) {
  return unwrap(await commands.reflog(repo, limit));
}

export async function lostCommits(repo: RepoId, limit = 100) {
  return unwrap(await commands.lostCommits(repo, limit));
}

export async function cherryPick(repo: RepoId, commits: string[]) {
  return unwrap(await commands.cherryPick(repo, commits));
}

export async function revertCommits(repo: RepoId, commits: string[]) {
  return unwrap(await commands.revert(repo, commits));
}

export async function rebaseOnto(repo: RepoId, options: RebaseOptions) {
  return unwrap(await commands.rebase(repo, options));
}

export async function skipOperation(repo: RepoId) {
  return unwrap(await commands.skipOperation(repo));
}

export async function mergeInto(repo: RepoId, options: MergeOptions) {
  return unwrap(await commands.merge(repo, options));
}

export async function remoteUrl(repo: RepoId, name: string) {
  return unwrap(await commands.remoteUrl(repo, name));
}

export async function listRemotes(repo: RepoId) {
  return unwrap(await commands.remotes(repo));
}

function progressChannel(onLine: (line: string) => void) {
  const channel = new Channel<string>();
  channel.onmessage = onLine;
  return channel;
}

export async function fetchRemote(repo: RepoId, remote: string, onLine: (line: string) => void) {
  return unwrap(await commands.fetch(repo, remote, progressChannel(onLine)));
}

export async function pullRemote(
  repo: RepoId,
  remote: string,
  ffOnly: boolean,
  onLine: (line: string) => void,
) {
  return unwrap(await commands.pull(repo, remote, ffOnly, progressChannel(onLine)));
}

export async function pushRemote(
  repo: RepoId,
  remote: string,
  force: boolean,
  onLine: (line: string) => void,
) {
  return unwrap(await commands.push(repo, remote, force, progressChannel(onLine)));
}

export async function createTag(repo: RepoId, request: TagRequest) {
  return unwrap(await commands.createTag(repo, request));
}

export async function deleteTag(repo: RepoId, name: string) {
  return unwrap(await commands.deleteTag(repo, name));
}

export async function listStashes(repo: RepoId) {
  return unwrap(await commands.stashes(repo));
}

export async function stashPush(repo: RepoId, options: StashOptions) {
  return unwrap(await commands.stashPush(repo, options));
}

export async function stashApply(repo: RepoId, index: number, pop: boolean) {
  return unwrap(await commands.stashApply(repo, index, pop));
}

export async function stashDrop(repo: RepoId, index: number) {
  return unwrap(await commands.stashDrop(repo, index));
}

export async function abortOperation(repo: RepoId) {
  return unwrap(await commands.abortOperation(repo));
}

export async function continueOperation(repo: RepoId) {
  return unwrap(await commands.continueOperation(repo));
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
