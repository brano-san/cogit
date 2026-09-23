import { Channel } from "@tauri-apps/api/core";

import { commands } from "./bindings";
import type {
  BlameChunk,
  BlameCommit,
  BlameSource,
  FileRevision,
  GitError,
  OriginEvent,
  OriginLine,
  OriginQuery,
  OriginReport,
  RepoId,
} from "./bindings";
import { CogitError } from "./index";

export type {
  BlameCommit,
  BlameSource,
  DeeperTarget,
  FileRevision,
  LineChange,
  LineMatch,
  Likelihood,
  OriginCandidate,
  OriginKind,
  OriginLine,
  OriginQuery,
  OriginReport,
  OriginText,
  PreviousFile,
} from "./bindings";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: GitError };

export interface BlameTables {
  commits: BlameCommit[];
  sources: BlameSource[];
  lines: OriginLine[];
}

/** Chunks may still be in flight when the command returns its count, so the promise
    settles on whichever of the two arrives last. */
function collect<T, C>(
  start: (channel: Channel<C>) => Promise<Result<number>>,
  take: (chunk: C) => void,
  size: () => number,
  done: () => T,
): Promise<T> {
  return new Promise((resolve, reject) => {
    let total: number | null = null;
    const settle = () => {
      if (total !== null && size() >= total) resolve(done());
    };
    const channel = new Channel<C>();
    channel.onmessage = (chunk) => {
      take(chunk);
      settle();
    };
    start(channel).then((result) => {
      if (result.status === "error") {
        reject(new CogitError(result.error));
        return;
      }
      total = result.data;
      settle();
    }, reject);
  });
}

export function investigateLog(
  repo: RepoId,
  path: string,
  rev: string | null,
  follow: boolean,
): Promise<FileRevision[]> {
  const rows: FileRevision[] = [];
  return collect<FileRevision[], FileRevision[]>(
    (channel) => commands.investigateLog(repo, path, rev, follow, channel),
    (chunk) => rows.push(...chunk),
    () => rows.length,
    () => rows,
  );
}

export function investigateBlame(
  repo: RepoId,
  path: string,
  rev: string | null,
  ignoreWhitespace: boolean,
): Promise<BlameTables> {
  const tables: BlameTables = { commits: [], sources: [], lines: [] };
  let header = false;
  return collect<BlameTables, BlameChunk>(
    (channel) => commands.investigateBlame(repo, path, rev, ignoreWhitespace, channel),
    (chunk) => {
      if (chunk.kind === "header") {
        header = true;
        tables.commits = chunk.commits;
        tables.sources = chunk.sources;
      } else {
        tables.lines.push(...chunk.lines);
      }
    },
    () => (header ? tables.lines.length : -1),
    () => tables,
  );
}

/** Resolves with the report, or `null` when the search was cancelled. `onStarted` hands
    over the id `cancelOriginSearch` needs. */
export function originCandidates(
  repo: RepoId,
  query: OriginQuery,
  onStarted: (id: number) => void,
): Promise<OriginReport | null> {
  return new Promise((resolve, reject) => {
    const channel = new Channel<OriginEvent>();
    channel.onmessage = (event) => {
      if (event.kind === "started") onStarted(event.id);
      else if (event.kind === "done") resolve(event.report);
      else resolve(null);
    };
    commands.originCandidates(repo, query, channel).then((result) => {
      if (result.status === "error") reject(new CogitError(result.error));
    }, reject);
  });
}

export async function cancelOriginSearch(id: number): Promise<void> {
  await commands.cancelOperation(id);
}

export async function openInvestigateWindow(url: string, title: string): Promise<void> {
  const result = await commands.openInvestigateWindow(url, title);
  if (result.status === "error") throw new CogitError(result.error);
}
