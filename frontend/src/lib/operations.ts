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
