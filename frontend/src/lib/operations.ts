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

/** A run over several repositories at once: the user started one thing, not N. */
export interface BulkProgress {
  label: string;
  done: number;
  total: number;
  failed?: number;
}

export interface ActivityInput {
  operations: ReadonlyMap<number, string>;
  bulk?: BulkProgress;
  network?: string;
  networkProgress?: string;
  opening: boolean;
  failed: boolean;
}

export interface Activity {
  label: string;
  busy: boolean;
  tone: "idle" | "busy" | "error";
}

/** One line for the whole app: the toolbar and the footer showed the same thing twice. */
export function activity(input: ActivityInput): Activity {
  const busy =
    (input.bulk === undefined ? null : describeBulk(input.bulk)) ??
    input.networkProgress ??
    (input.network === undefined ? null : `${input.network}…`) ??
    busyLabel(input.operations) ??
    (input.opening ? "Opening repository…" : null);

  if (busy !== null) return { label: busy, busy: true, tone: "busy" };
  if (input.failed) return { label: "Error", busy: false, tone: "error" };
  return { label: "Ready", busy: false, tone: "idle" };
}

function describeBulk(bulk: BulkProgress): string {
  const failed = bulk.failed ? ` (${bulk.failed} failed)` : "";
  return `${bulk.label} ${bulk.done} of ${bulk.total}…${failed}`;
}
