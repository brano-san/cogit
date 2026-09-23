import { decodeBase64Window } from "$lib/graph-wire";
import { Channel } from "@tauri-apps/api/core";

import { commands, events } from "./bindings";
import { counted } from "$lib/listener-count";
import { mergeUrl } from "$lib/merge-params";
import type {
  Author,
  AvatarReady,
  CheckoutTarget,
  CommitQuery,
  CommitRequest,
  ConfigScope,
  ConflictSide,
  ContextItem,
  DiffOptions,
  DiffSpec,
  FlowConfig,
  FlowKind,
  GitError,
  GraphProgress,
  MergeOptions,
  MergeResolved,
  ModuleProblem,
  CommandNotice,
  OperationChanged,
  PatchRequest,
  RebaseOptions,
  RendererMemory,
  RepoChanged,
  RepoId,
  ScanHit,
  SearchChunk,
  SearchScope,
  WorktreeView,
  StashOptions,
  TagRequest,
  TodoEntry,
} from "./bindings";

export type {
  Algorithm,
  AppInfo,
  DisplayInfo,
  OsInfo,
  ConfigFile,
  ConfigScope,
  HealthFinding,
  HealthIssue,
  ModuleProblem,
  Operation,
  OperationKind,
  SkippedRef,
  OperationPhase,
  BlameLine,
  Branch,
  BranchKind,
  Bypass,
  ChangeKind,
  CheckoutTarget,
  CommitDetails,
  CommitQuery,
  CommitRequest,
  CommandNotice,
  CommitRow,
  ConflictSide,
  Author,
  AvatarReady,
  AvatarRow,
  ContextItem,
  DiffOptions,
  DiffRow,
  DiffSpec,
  EolInfo,
  FileDiff,
  FileEntry,
  FileStatus,
  FlowBranch,
  FlowConfig,
  FlowKind,
  FlowStatus,
  Found,
  FoundKind,
  GitCommandError,
  GitError,
  GitOutput,
  GraphProgress,
  GraphRow,
  Head,
  Hook,
  HookOverview,
  HookRun,
  HookSource,
  HookState,
  Hunk,
  Segment,
  Span,
  LineEnding,
  LineVersion,
  MergeOptions,
  MergeResolved,
  OperationChanged,
  Origin,
  Overlap,
  OverlapRow,
  PresetStatus,
  PatchRequest,
  RebaseOptions,
  RebaseProgress,
  Region,
  KeyBinding,
  RebaseStep,
  RefDate,
  ReflogEntry,
  ScanHit,
  ContentMatch,
  SearchChunk,
  SearchScope,
  StashContents,
  WorktreeEntry,
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
  TodoAction,
  TodoEntry,
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
    case "moduleUnavailable":
      return describeModuleProblem(error.data);
    case "configInvalid":
      return error.data.line === null
        ? `Git refused the config: ${error.data.message}`
        : `Git refused the config at line ${error.data.line}: ${error.data.message}`;
  }
}

/** The backend's wording, in the one place the frontend writes it (R-149). */
export function describeModuleProblem(problem: ModuleProblem): string {
  switch (problem.reason) {
    case "missing":
      return `Directory does not exist: ${problem.path}`;
    case "notInitialised":
      return `Submodule is not initialised: ${problem.path}`;
    case "danglingGitFile":
      return problem.foreign
        ? `The .git file of ${problem.path} points to ${problem.target}, which does not exist on this system — the path was written by another operating system.`
        : `The .git file of ${problem.path} points to ${problem.target}, which does not exist.`;
    case "notARepository":
      return `Not a Git repository: ${problem.path}: ${problem.detail}`;
  }
}

export async function getAppInfo() {
  return unwrap(await commands.appInfo());
}

/** `frontend` is the list the Vite build shipped; null under the dev server. */
export async function openThirdPartyLicences(frontend: string | null) {
  unwrap(await commands.openThirdPartyLicences(frontend));
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
  visibleRefs: null,
};

/** The walk reports how far it got; the rows stay in Rust until `graphWindow` (R-193). */
export async function loadGraph(
  repo: RepoId,
  onProgress: (progress: GraphProgress) => void,
  query: CommitQuery = EMPTY_QUERY,
) {
  const channel = new Channel<GraphProgress>();
  channel.onmessage = onProgress;
  return unwrap(await commands.loadCommits(repo, query, channel));
}

/** `null` once a newer walk replaced `generation`. Rows are decoded when drawn (R-194). */
export async function graphWindow(repo: RepoId, generation: number, start: number, count: number) {
  return decodeBase64Window(unwrap(await commands.graphWindow(repo, generation, start, count)));
}

export async function graphRowOf(repo: RepoId, generation: number, oid: string) {
  return unwrap(await commands.graphRowOf(repo, generation, oid));
}

/** Hits stream in as the walk finds them; the promise resolves with the total. */
export async function scanForRepositories(
  path: string,
  maxDepth: number,
  onFound: (hit: ScanHit) => void,
) {
  const channel = new Channel<ScanHit>();
  channel.onmessage = onFound;
  return unwrap(await commands.scanForRepositories(path, maxDepth, channel));
}

/** Matches stream in; the first chunk carries the id `cancelOperation` takes. */
export async function searchFileContents(
  repo: RepoId,
  query: string,
  isRegex: boolean,
  scope: SearchScope,
  onChunk: (chunk: SearchChunk) => void,
) {
  const channel = new Channel<SearchChunk>();
  channel.onmessage = onChunk;
  return unwrap(await commands.searchFileContents(repo, query, isRegex, scope, channel));
}

/** `false` when the operation had already finished. */
export async function cancelOperation(id: number) {
  return commands.cancelOperation(id);
}

export async function commitTreeFiles(repo: RepoId, rev: string) {
  return unwrap(await commands.commitTreeFiles(repo, rev));
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

export async function worktreeFiles(repo: RepoId, view: WorktreeView) {
  return unwrap(await commands.worktreeFiles(repo, view));
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

export async function renameBranch(repo: RepoId, from: string, to: string, force: boolean) {
  return unwrap(await commands.renameBranch(repo, from, to, force));
}

/** `null` stops the branch tracking anything rather than pointing it somewhere harmless. */
export async function setUpstream(repo: RepoId, branch: string, upstream: string | null) {
  return unwrap(await commands.setUpstream(repo, branch, upstream));
}

export async function deleteRemoteBranch(repo: RepoId, remote: string, branch: string) {
  return unwrap(await commands.deleteRemoteBranch(repo, remote, branch));
}

/** Fires when the watcher sees the repository change on disk; returns an unlisten fn. */
export async function onRepoChanged(handler: (change: RepoChanged) => void) {
  return await counted(events.repoChanged.listen((event) => handler(event.payload)));
}

export async function commandLog() {
  return await commands.commandLog();
}

/** One record in full. A notice carries its summary; this is where the output lives. */
export async function commandOutcome(id: number) {
  return await commands.commandOutcome(id);
}

/** Fires once per git command, whatever it did; the payload says how it went, not what
 *  it printed. */
export async function onCommandRecorded(handler: (event: CommandNotice) => void) {
  return await counted(events.commandRecorded.listen((event) => handler(event.payload)));
}

export async function safetyLog() {
  return await commands.safetyLog();
}

export async function runCheck(repo: RepoId, command: string) {
  return unwrap(await commands.runCheck(repo, command));
}

export async function listWorktrees(repo: RepoId) {
  return unwrap(await commands.worktrees(repo));
}

export async function worktreeHolding(repo: RepoId, branch: string) {
  return unwrap(await commands.worktreeHolding(repo, branch));
}

export async function addWorktree(
  repo: RepoId,
  path: string,
  branch: string,
  create: boolean,
  base: string | null = null,
) {
  return unwrap(await commands.addWorktree(repo, path, branch, create, base));
}

export async function removeWorktree(repo: RepoId, path: string, force: boolean) {
  return unwrap(await commands.removeWorktree(repo, path, force));
}

export async function pruneWorktrees(repo: RepoId) {
  return unwrap(await commands.pruneWorktrees(repo));
}

export async function openWorktree(owner: RepoId, path: string) {
  return unwrap(await commands.openWorktree(owner, path));
}

export async function worktreeChanges(repo: RepoId, path: string) {
  return unwrap(await commands.worktreeChanges(repo, path));
}

export async function pruneWorktree(repo: RepoId, path: string) {
  return unwrap(await commands.pruneWorktree(repo, path));
}

export async function repairWorktree(repo: RepoId, path: string) {
  return unwrap(await commands.repairWorktree(repo, path));
}

export async function lockWorktree(repo: RepoId, path: string, reason: string | null) {
  return unwrap(await commands.lockWorktree(repo, path, reason));
}

export async function unlockWorktree(repo: RepoId, path: string) {
  return unwrap(await commands.unlockWorktree(repo, path));
}

export async function terminalChoices() {
  return await commands.terminalChoices();
}

export async function openInTerminal(path: string, terminal: string) {
  return unwrap(await commands.openInTerminal(path, terminal));
}

export async function stashSelection(repo: RepoId, paths: string[], message: string) {
  return unwrap(await commands.stashSelection(repo, paths, message));
}

export async function stashContents(repo: RepoId, index: number) {
  return unwrap(await commands.stashContents(repo, index));
}

export async function undoEntry(repo: RepoId, id: number) {
  return unwrap(await commands.undoEntry(repo, id));
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

/** `rev` may be any revision; the window is titled with the commit it resolves to. */
export async function openBlameWindow(repo: RepoId, path: string, rev: string) {
  return unwrap(await commands.openBlameWindow(repo, path, rev));
}

export async function fileRevisions(repo: RepoId, path: string, rev: string) {
  return unwrap(await commands.fileRevisions(repo, path, rev));
}

export async function lineHistory(repo: RepoId, path: string, rev: string, line: number) {
  return unwrap(await commands.lineHistory(repo, path, rev, line));
}

export async function stageSelection(repo: RepoId, request: PatchRequest, reverse: boolean) {
  return unwrap(await commands.stageSelection(repo, request, reverse));
}

/** The submodules directly under `parent`; `""` is the repository itself. Read one node
 *  at a time: a tree nine deep must not cost nine reads before anyone expands anything. */
export async function listSubmodules(repo: RepoId, parent = "") {
  return unwrap(await commands.listSubmodules(repo, parent));
}

/** Opens a submodule from its node: the panels follow it, the list does not grow. */
export async function readGitConfig(repo: RepoId | null, scope: ConfigScope) {
  return unwrap(await commands.readGitConfig(repo === null ? null : repo.valueOf(), scope));
}

export async function writeGitConfig(
  repo: RepoId | null,
  scope: ConfigScope,
  text: string,
  crlf: boolean,
) {
  return unwrap(
    await commands.writeGitConfig(repo === null ? null : repo.valueOf(), scope, text, crlf),
  );
}

/** The repository and every submodule below it; read-only apart from a probe file. */
export async function repositoryHealth(repo: RepoId) {
  return unwrap(await commands.repositoryHealth(repo));
}

export async function listOperations() {
  return unwrap(await commands.listOperations());
}

/** `key` is the node path from `owner`, the repository whose tree it is (R-149). */
export async function openSubmodule(owner: RepoId, key: string) {
  return unwrap(await commands.openSubmodule(owner, key));
}

export async function updateSubmodule(repo: RepoId, path: string, init: boolean) {
  return unwrap(await commands.updateSubmodule(repo, path, init));
}

export async function listRepositories() {
  return await commands.repositories();
}

export async function closeRepository(repo: RepoId) {
  return unwrap(await commands.closeRepository(repo));
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

export async function refDates(repo: RepoId) {
  return unwrap(await commands.refDates(repo));
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

/** Fires when a native menu item is chosen; the payload is a palette command id. */
export async function onMenuCommand(handler: (id: string) => void) {
  return await counted(events.menuCommand.listen((event) => handler(event.payload)));
}

/** Windows wants to end the session and was told to wait for the queue (R-168). */
export async function onSessionEnding(handler: () => void) {
  return await counted(events.sessionEnding.listen(() => handler()));
}

/** The shipped accelerators, kept beside the menu they belong to. */
export async function defaultKeymap() {
  return await commands.defaultKeymap();
}

/** Rebuilds the native menu bar with the user's keys. */
export async function setKeymap(overrides: Record<string, string>) {
  return unwrap(await commands.setKeymap(overrides));
}

/** Lands in the profile log beside the backend's own numbers (F-116). */
export async function reportTiming(label: string, ms: number, detail: string) {
  return await commands.reportTiming(label, ms, detail);
}

/** One renderer memory sample; `kind=mem` in the profile log (doc/14-profiling.md). */
export async function reportMemory(sample: RendererMemory) {
  return await commands.reportMemory(sample);
}

/** The settings document as JSON text; Rust owns the file (R-81). */
export async function readSettings(): Promise<string> {
  return await commands.readSettings();
}

export async function writeSetting(key: string, value: string): Promise<null> {
  return unwrap(await commands.writeSetting(key, value));
}

export async function setMenuState(disabled: string[], checked: string[]) {
  return await commands.setMenuState(disabled, checked);
}

export async function hasToken(host: string) {
  return unwrap(await commands.hasToken(host));
}

export async function storeToken(host: string, token: string) {
  return unwrap(await commands.storeToken(host, token));
}

export async function forgetToken(host: string) {
  return unwrap(await commands.forgetToken(host));
}

export async function listHooks(repo: RepoId) {
  return unwrap(await commands.listHooks(repo));
}

export async function readHook(repo: RepoId, name: string) {
  return unwrap(await commands.readHook(repo, name));
}

export async function writeHook(repo: RepoId, name: string, body: string) {
  return unwrap(await commands.writeHook(repo, name, body));
}

export async function setHookEnabled(repo: RepoId, name: string, enabled: boolean) {
  return unwrap(await commands.setHookEnabled(repo, name, enabled));
}

export async function useHooksPath(repo: RepoId, path: string) {
  return unwrap(await commands.useHooksPath(repo, path));
}

export async function runHook(repo: RepoId, name: string) {
  return unwrap(await commands.runHook(repo, name));
}

export async function rollbackTo(repo: RepoId, rev: string, paths: string[]) {
  return unwrap(await commands.rollbackTo(repo, rev, paths));
}

/** The shared branches holding this commit; empty means rewriting it costs nobody. */
export async function protectingRefs(repo: RepoId, rev: string) {
  return unwrap(await commands.protectingRefs(repo, rev));
}

export async function isPublished(repo: RepoId, rev: string) {
  return unwrap(await commands.isPublished(repo, rev));
}

export async function splitOff(
  repo: RepoId,
  rev: string,
  paths: string[],
  message: string,
  splitFirst: boolean,
) {
  return unwrap(await commands.splitOff(repo, rev, paths, message, splitFirst));
}

export async function rebaseTodo(repo: RepoId, base: string) {
  return unwrap(await commands.rebaseTodo(repo, base));
}

export async function interactiveRebase(
  repo: RepoId,
  base: string,
  plan: TodoEntry[],
  paused: boolean,
) {
  return unwrap(await commands.interactiveRebase(repo, base, plan, paused));
}

export async function rebaseProgress(repo: RepoId) {
  return unwrap(await commands.rebaseProgress(repo));
}

export async function overlapWindow(repo: RepoId, base: string, window: string[]) {
  return unwrap(await commands.overlapWindow(repo, base, window));
}

export async function bypassLog(repo: RepoId) {
  return unwrap(await commands.bypassLog(repo));
}

/** Git-Flow, over ordinary git commands: the extension is not required (M5 T5.5). */
export async function flowStatus(repo: RepoId) {
  return unwrap(await commands.flowStatus(repo));
}

export async function flowInit(repo: RepoId, config: FlowConfig) {
  return unwrap(await commands.flowInit(repo, config));
}

export async function flowStart(repo: RepoId, kind: FlowKind, name: string) {
  return unwrap(await commands.flowStart(repo, kind, name));
}

export async function flowFinish(
  repo: RepoId,
  kind: FlowKind,
  name: string,
  tag: string | null,
) {
  return unwrap(await commands.flowFinish(repo, kind, name, tag));
}

/** Opens one conflicted file in a window of its own. */
export async function openMergeWindow(repo: RepoId, path: string) {
  return unwrap(await commands.openMergeWindow(mergeUrl(repo, path), `${path} — Cogit`));
}

/** Told by the merge window once the resolution is written. */
export async function mergeResolved(repo: RepoId, path: string) {
  return unwrap(await commands.mergeResolved(repo, path));
}

export async function onMergeResolved(handler: (event: MergeResolved) => void) {
  return await counted(events.mergeResolved.listen((event) => handler(event.payload)));
}

/** The three sides already merged into regions, for the four-panel merge view. */
export async function mergePreview(repo: RepoId, path: string) {
  return unwrap(await commands.mergePreview(repo, path));
}

/** The addresses on screen. Cheap enough to send on every scroll: it only moves the
    download queue and reads nothing. */
export async function avatarWindow(emails: string[]) {
  return unwrap(await commands.avatarWindow(emails));
}

/** The pictures these authors already have. Costs a file read each, so ask narrowly. */
export async function avatarsFor(authors: Author[]) {
  return unwrap(await commands.avatars(authors));
}

export async function setAvatars(enabled: boolean) {
  return unwrap(await commands.setAvatars(enabled));
}

/** Fires when one author's picture has landed in the cache and the row can redraw. */
export async function onAvatarReady(handler: (event: AvatarReady) => void) {
  return await counted(events.avatarReady.listen((event) => handler(event.payload)));
}

/** Fires when a tracked operation starts or finishes; drives the toolbar spinner. */
export async function onOperationChanged(handler: (event: OperationChanged) => void) {
  return await counted(events.operationChanged.listen((event) => handler(event.payload)));
}

export async function popupContextMenu(items: ContextItem[], x: number, y: number) {
  return unwrap(await commands.popupContextMenu(items, x, y));
}

/** Closes the window this webview lives in. The close happens in Rust: a window
 *  operation issued from inside the webview is what R-86 is about. */
export async function closeThisWindow() {
  return unwrap(await commands.closeThisWindow());
}

export async function openCompareWindow(url: string, title: string) {
  return unwrap(await commands.openCompareWindow(url, title));
}

export async function commitTemplate(repo: RepoId) {
  return unwrap(await commands.commitTemplate(repo));
}

export async function stageMode(repo: RepoId, path: string, executable: boolean) {
  return unwrap(await commands.stageMode(repo, path, executable));
}

export async function listPresets(repo: RepoId) {
  return unwrap(await commands.listPresets(repo));
}

/** Saves the hook as it stands as a preset of the user's own. */
export async function exportPreset(
  repo: RepoId,
  hook: string,
  id: string,
  name: string,
  description: string,
) {
  return unwrap(await commands.exportPreset(repo, hook, id, name, description));
}

export async function removePreset(id: string) {
  return unwrap(await commands.removePreset(id));
}

export async function installPreset(repo: RepoId, id: string) {
  return unwrap(await commands.installPreset(repo, id));
}

/** Every file of a commit in one round trip. Answers in the order the paths were given.
    `request` must rise with every selection: an older one comes back `superseded`. */
export async function diffFiles(
  repo: RepoId,
  spec: DiffSpec,
  paths: string[],
  options: DiffOptions,
  request: number,
) {
  return unwrap(await commands.diffFiles(repo, spec, paths, options, request));
}

/** The types the branch added. The list above the divider belongs to `master`. */
export type { DiffBatch, FileDiffEntry, InvestigationStep, MoveScope } from "./bindings";

/** Throws the selected lines away in the working tree. Destructive: confirm it first. */
export async function discardSelection(repo: RepoId, request: PatchRequest) {
  return unwrap(await commands.discardSelection(repo, request));
}

/** The file as it was before a commit; `null` when there was no such file to open. */
export async function fileBefore(repo: RepoId, oid: string, path: string) {
  return unwrap(await commands.fileBefore(repo, oid, path));
}

/** Every commit that changed lines `from..=to` of a file, newest first. */
export async function investigate(
  repo: RepoId,
  path: string,
  from: number,
  to: number,
  limit: number,
) {
  return unwrap(await commands.investigate(repo, path, from, to, limit));
}
