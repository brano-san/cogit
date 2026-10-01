/** What `probe_git` answers, as the IPC layer delivers it. */
export interface Probe {
  valid: boolean;
  version: string | null;
  error: string | null;
}

export type GitCheck =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "ok"; version: string }
  | { state: "bad"; reason: string };

export function describeCheck(check: GitCheck): string {
  switch (check.state) {
    case "ok":
      return `Version: ${check.version}`;
    case "bad":
      return check.reason;
    case "checking":
      return "Checking…";
    default:
      return "";
  }
}

/** Debounces typing; an answer that a later check has overtaken is dropped, so the mark
    never shows the verdict on a path that is no longer in the field. */
export function createGitChecker(
  probe: (path: string) => Promise<Probe>,
  onchange: (check: GitCheck) => void,
  delay = 400,
) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let latest = 0;

  return {
    check(path: string) {
      clearTimeout(timer);
      const mine = ++latest;
      onchange({ state: "checking" });
      timer = setTimeout(async () => {
        let next: GitCheck;
        try {
          const answer = await probe(path);
          next =
            answer.valid && answer.version !== null
              ? { state: "ok", version: answer.version }
              : { state: "bad", reason: answer.error ?? "Not a working git." };
        } catch (err) {
          next = { state: "bad", reason: err instanceof Error ? err.message : String(err) };
        }
        if (mine === latest) onchange(next);
      }, delay);
    },
    dispose() {
      clearTimeout(timer);
      latest++;
    },
  };
}

export const ON_WINDOWS = typeof navigator !== "undefined" && navigator.platform.startsWith("Win");
