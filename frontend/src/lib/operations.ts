import type { BackendChanged, OperationPhase } from "./ipc";

export interface OperationEvent {
  id: number;
  label: string;
  success: boolean | null;
}

export function applyOperation(
  running: ReadonlyMap<number, string>,
  event: OperationEvent,
): Map<number, string> {
  const next = new Map(running);
  if (event.success === null) next.set(event.id, event.label);
  else next.delete(event.id);
  return next;
}

export function busyLabel(running: ReadonlyMap<number, string>): string | null {
  if (running.size === 0) return null;
  if (running.size === 1) return `${[...running.values()][0]}…`;
  return `${running.size} operations running…`;
}

/** The ids of the operations running now that `cancel_network` can stop, in the order they
    started (R-506): what the backend registered a stop for, whatever its kind. One still
    waiting in the queue has no git to stop yet. */
export function trackCancellable(
  ids: readonly number[],
  event: { id: number; phase: OperationPhase; cancellable: boolean },
): number[] {
  const rest = ids.filter((id) => id !== event.id);
  if (event.phase === "running" && event.cancellable) rest.push(event.id);
  return rest;
}

/** The one the footer's Cancel stops: the last to start, the one its label names. */
export function cancellable(ids: readonly number[]): number | null {
  return ids.at(-1) ?? null;
}

/** A run over several repositories at once: the user started one thing, not N. */
export interface BulkProgress {
  label: string;
  done: number;
  total: number;
  failed?: number;
}

export interface ActivityInput {
  operations: ReadonlyMap<number, string>;
  update?: string;
  bulk?: BulkProgress;
  network?: string;
  networkProgress?: string;
  /** Hooks git runs inside the step on screen (D4). */
  hooks?: string;
  opening: boolean;
  /** Hosts whose agent is starting, as `BackendChanged` names them. */
  starting?: readonly string[];
  failed: boolean;
}

export interface Activity {
  label: string;
  busy: boolean;
  tone: "idle" | "busy" | "error";
  /** How far a busy step is, 0..1, when it says: git's percent or a batch's count. */
  fraction: number | null;
}

export function trackStarting(starting: readonly string[], event: BackendChanged): string[] {
  const others = starting.filter((host) => host !== event.host);
  return event.state.state === "starting" ? [...others, event.host] : others;
}

/** One line for the whole app: the toolbar and the footer showed the same thing twice. */
export function activity(input: ActivityInput): Activity {
  const busy =
    input.update ??
    (input.bulk === undefined ? null : describeBulk(input.bulk)) ??
    input.networkProgress ??
    (input.network === undefined ? null : `${input.network}…`) ??
    busyLabel(input.operations) ??
    (input.starting?.length ? `Starting ${input.starting.join(", ")}…` : null) ??
    (input.opening ? "Opening repository…" : null);

  if (busy !== null) {
    const fraction = input.bulk ? bulkFraction(input.bulk) : percentOf(busy);
    return { label: input.hooks ? `${busy} — ${input.hooks}` : busy, busy: true, tone: "busy", fraction };
  }
  if (input.failed) return { label: "Error", busy: false, tone: "error", fraction: null };
  return { label: "Ready", busy: false, tone: "idle", fraction: null };
}

function bulkFraction(bulk: BulkProgress): number | null {
  return bulk.total > 0 ? Math.min(bulk.done / bulk.total, 1) : null;
}

/** git's progress line names its percent ("Receiving objects:  40% (4/10)"): the last one. */
export function percentOf(label: string): number | null {
  const found = [...label.matchAll(/(\d{1,3})%/g)].at(-1);
  if (!found) return null;
  return Math.min(Number(found[1]), 100) / 100;
}

function describeBulk(bulk: BulkProgress): string {
  const failed = bulk.failed ? ` (${bulk.failed} failed)` : "";
  return `${bulk.label} ${bulk.done} of ${bulk.total}…${failed}`;
}
