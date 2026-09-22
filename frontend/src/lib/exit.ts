import type { Operation } from "$lib/ipc";

export function exitBlockers(
  operations: readonly Operation[],
  repoNames: ReadonlyMap<number, string>,
): string[] {
  return operations
    .filter((operation) => operation.phase !== "done")
    .map((operation) => {
      const state = operation.phase === "running" ? "running" : "waiting";
      const name = operation.repo === null ? undefined : repoNames.get(operation.repo.valueOf());
      return name === undefined
        ? `${operation.label} (${state})`
        : `${operation.label} — ${name} (${state})`;
    });
}

/** "Don't show again" silences the question, never the warning that work would be lost. */
export function mustAskBeforeExit(confirmExit: boolean, unfinished: number): boolean {
  return confirmExit || unfinished > 0;
}
