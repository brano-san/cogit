import { CogitError, type GitCommandError, type GitError, type GitOutput } from "$lib/ipc";
import type { HealthAction, HealthPlace, HealthWarning } from "$lib/health";
import { logLines } from "$lib/output-highlight";

/** `info` is how an operation the user started ended; it never counts as an error. */
export type NoticeSeverity = "error" | "warning" | "info";

/** One entry of the notification window: a failure, or something wrong with a repository. */
export interface Notice {
  key: string;
  severity: NoticeSeverity;
  title: string;
  body: string;
  /** Raw git output, shown whole; absent when there is none or it is too long for here. */
  output?: string;
  outputLines?: number;
  places?: HealthPlace[];
  fixes?: string[];
  docs?: string;
  /** What Copy puts on the clipboard: everything someone would have to ask for anyway. */
  report: string;
  record?: number;
  warning?: string;
  action?: HealthAction;
  repeats: number;
}

/** Past this the output belongs in the output window, which is virtualized (R-90). */
export const OUTPUT_LINES_SHOWN = 400;

export const PLACES_SHOWN = 5;

type CommandRun = Pick<GitCommandError, "command" | "exitCode" | "stdout" | "stderr" | "repo"> &
  Partial<Pick<GitOutput, "durationMs" | "startedAtMs">>;

export function repoNameOf(root: string): string {
  return root.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? root;
}

export function commandReport(run: CommandRun): string {
  const facts = [`Command: ${run.command}`, `Exit code: ${run.exitCode ?? "did not start"}`];
  if (run.durationMs !== undefined) facts.push(`Duration: ${run.durationMs} ms`);
  if (run.startedAtMs !== undefined) {
    facts.push(`Started: ${new Date(run.startedAtMs).toLocaleString()}`);
  }
  facts.push(`Repository: ${repoNameOf(run.repo)}`);
  return [...facts, "", ...logLines(run.stdout, run.stderr).map((line) => line.text)].join("\n");
}

export function commandNotice(
  run: CommandRun & Pick<GitCommandError, "id" | "operation" | "summary">,
): Notice {
  const text = [run.stderr, run.stdout].filter((part) => part.trim() !== "").join("\n");
  const lines = text === "" ? 0 : text.trimEnd().split("\n").length;
  const notice: Notice = {
    key: `command:${run.id}`,
    severity: "error",
    title: `${run.operation} failed`,
    body: run.summary,
    report: commandReport(run),
    record: run.id,
    repeats: 1,
  };
  if (lines > OUTPUT_LINES_SHOWN) notice.outputLines = lines;
  else if (lines > 0) notice.output = text.trimEnd();
  return notice;
}

export function asCogitError(value: unknown): CogitError | null {
  if (value === null || value === undefined) return null;
  if (value instanceof CogitError) return value;
  if (typeof value === "object" && "kind" in value && "data" in value) {
    return new CogitError(value as GitError);
  }
  const text =
    value instanceof Error
      ? value.message
      : typeof value === "object" && "message" in value
        ? String((value as { message: unknown }).message)
        : String(value);
  return new CogitError({ kind: "invalidState", data: text });
}

/** `title` says what did not work — the operation, never "Cogit stopped" (R-178). */
export function errorNotice(error: CogitError, title: string, seq: number): Notice {
  if (error.detail.kind === "command") return commandNotice(error.detail.data);
  return {
    key: `error:${seq}`,
    severity: "error",
    title,
    body: error.message,
    report: `${title}\n${error.message}`,
    repeats: 1,
  };
}

export function warningNotice(warning: HealthWarning): Notice {
  const lines = [
    warning.title,
    "",
    warning.body,
    "",
    ...warning.places.map((place) =>
      place.detail ? `${place.label}  ${place.detail}` : place.label,
    ),
  ];
  if (warning.fixes.length > 0) lines.push("", ...warning.fixes);
  const notice: Notice = {
    key: `warning:${warning.id}`,
    severity: "warning",
    title: warning.title,
    body: warning.body,
    places: warning.places,
    fixes: warning.fixes,
    docs: warning.docs,
    report: lines.join("\n"),
    warning: warning.id,
    repeats: 1,
  };
  if (warning.action) notice.action = warning.action;
  return notice;
}

/** Newest first; an entry already queued is not queued twice, and the same failure again
    is a count on the one already there, moved to the front. */
export function pushError(errors: readonly Notice[], notice: Notice): Notice[] {
  if (errors.some((held) => held.key === notice.key)) return [...errors];
  const same = errors.find((held) => held.title === notice.title && held.body === notice.body);
  if (!same) return [notice, ...errors];
  const counted = notice.record === undefined ? same.repeats : same.repeats + 1;
  return [{ ...notice, repeats: counted }, ...errors.filter((held) => held !== same)];
}

export function queueOf(
  errors: readonly Notice[],
  warnings: readonly Notice[],
  results: readonly Notice[] = [],
): Notice[] {
  return [...errors, ...warnings, ...results];
}

export function placesShown(
  places: readonly HealthPlace[],
  expanded: boolean,
): { shown: readonly HealthPlace[]; more: number } {
  if (expanded || places.length <= PLACES_SHOWN + 1) return { shown: places, more: 0 };
  return { shown: places.slice(0, PLACES_SHOWN), more: places.length - PLACES_SHOWN };
}
