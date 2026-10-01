import type {
  FileEntry,
  GitError,
  UnpushedInSubmodule,
  WorktreeEntry,
  WorktreeScanChunk,
  WorktreeSubmodules,
} from "$lib/ipc";
import { removalNeeds } from "$lib/worktree-list";

/** One stage of the Remove Worktree check: running, then its answer or the error. */
export type Stage<T> =
  | { status: "running" }
  | { status: "done"; value: T }
  | { status: "failed"; error: GitError };

export interface ScanState {
  changes: Stage<FileEntry[]>;
  submodules: Stage<WorktreeSubmodules>;
  unpushed: Stage<UnpushedInSubmodule[]>;
}

export type StageId = keyof ScanState;

export interface StageRow {
  id: StageId;
  label: string;
  status: Stage<unknown>["status"];
  detail: string;
}

const STAGES: readonly StageId[] = ["changes", "submodules", "unpushed"];

const LABELS: Record<StageId, string> = {
  changes: "Changes",
  submodules: "Submodules",
  unpushed: "Unpushed commits",
};

export function startScan(): ScanState {
  return {
    changes: { status: "running" },
    submodules: { status: "running" },
    unpushed: { status: "running" },
  };
}

/** The backend answers each stage when it finishes, in any order. */
export function applyChunk(scan: ScanState, chunk: WorktreeScanChunk): ScanState {
  switch (chunk.kind) {
    case "changes":
      return { ...scan, changes: { status: "done", value: chunk.files } };
    case "submodules":
      return { ...scan, submodules: { status: "done", value: chunk.modules } };
    case "unpushed":
      return { ...scan, unpushed: { status: "done", value: chunk.found } };
    case "failed":
      return { ...scan, [chunk.stage]: { status: "failed", error: chunk.error } };
    default:
      return scan;
  }
}

export function scanSettled(scan: ScanState): boolean {
  return STAGES.every((id) => scan[id].status !== "running");
}

function failed(scan: ScanState): boolean {
  return STAGES.some((id) => scan[id].status === "failed");
}

function reason(error: GitError): string {
  const data = (error as { data?: unknown }).data;
  return typeof data === "string" ? data : error.kind;
}

function plural(count: number, one: string): string {
  return `${count} ${count === 1 ? one : `${one}s`}`;
}

export function unpushedTotal(scan: ScanState): number {
  return scan.unpushed.status === "done"
    ? scan.unpushed.value.reduce((sum, item) => sum + item.total, 0)
    : 0;
}

function detail(scan: ScanState, id: StageId): string {
  const stage = scan[id];
  if (stage.status === "running") return "";
  if (stage.status === "failed") return reason(stage.error);
  if (id === "changes") {
    const count = scan.changes.status === "done" ? scan.changes.value.length : 0;
    return count === 0 ? "none" : plural(count, "file");
  }
  if (id === "submodules") {
    const count = scan.submodules.status === "done" ? scan.submodules.value.paths.length : 0;
    return count === 0 ? "none" : `${count} checked out`;
  }
  const found = scan.unpushed.status === "done" ? scan.unpushed.value.length : 0;
  return found === 0 ? "none" : `${unpushedTotal(scan)} in ${plural(found, "submodule")}`;
}

export function stageRows(scan: ScanState): StageRow[] {
  return STAGES.map((id) => ({
    id,
    label: LABELS[id],
    status: scan[id].status,
    detail: detail(scan, id),
  }));
}

/** Edits and the submodules that changed, as one list; `null` until both are known. */
export function uncommittedList(scan: ScanState): FileEntry[] | null {
  if (scan.changes.status !== "done" || scan.submodules.status !== "done") return null;
  return [...scan.changes.value, ...scan.submodules.value.changed].sort((a, b) =>
    a.path < b.path ? -1 : a.path > b.path ? 1 : 0,
  );
}

export interface RemoveButton {
  label: string;
  disabled: boolean;
  tip: string | undefined;
}

/** `--force` is needed for changes and for submodules; the box answers it. */
export function removeButton(entry: WorktreeEntry, scan: ScanState, force: boolean): RemoveButton {
  const needs = removalNeeds(entry, uncommittedList(scan));
  const label = needs.force ? "Remove with --force" : "Remove";
  if (failed(scan)) return { label, disabled: true, tip: "The check failed" };
  if (!scanSettled(scan)) return { label, disabled: true, tip: "Waiting for the check" };
  if (needs.force && !force) {
    return { label, disabled: true, tip: "Tick Remove anyway (--force) first" };
  }
  return { label, disabled: false, tip: undefined };
}
