import type { Flash, Progress, TaskbarSignals } from "$lib/ipc";

/** The last percentage Git printed ("Receiving objects:  45% (9/20)"), or null. */
export function percentOf(line: string | null): number | null {
  if (line === null) return null;
  const all = [...line.matchAll(/(\d{1,3})%/g)];
  const last = all.at(-1);
  return last ? Math.min(100, Number(last[1])) : null;
}

export function progressOf(running: boolean, line: string | null): Progress | null {
  if (!running) return null;
  const value = percentOf(line);
  return value === null ? { kind: "indeterminate" } : { kind: "percent", value };
}

export interface TaskbarInputs {
  /** Preferences ▸ Notifications: the whole indication. */
  enabled: boolean;
  /** Preferences ▸ Notifications: the flash alone. */
  flashEnabled: boolean;
  running: boolean;
  line: string | null;
  errors: number;
  warnings: number;
  unviewed: number;
  flash: Flash | null;
}

/** Switched off means "show nothing", not "stop updating": the last state must be cleared. */
export function signalsFor(input: TaskbarInputs): TaskbarSignals {
  if (!input.enabled) return {};
  return {
    progress: progressOf(input.running, input.line),
    errors: input.errors,
    warnings: input.warnings,
    unviewed: input.unviewed,
    flash: input.flashEnabled ? input.flash : null,
  };
}
