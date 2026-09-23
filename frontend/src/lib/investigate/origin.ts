import type { OriginCandidate, OriginReport } from "$lib/ipc/investigate";
import { fileName } from "./params";

/** What the card and the candidate list call a candidate. `blockPath` is the file the
    block was in just before the commit, so a source in it reads "within the file". */
export function describeCandidate(
  candidate: OriginCandidate,
  blockPath: string,
): { title: string; detail: string } {
  const here = candidate.path === blockPath;
  const where = here ? `line ${candidate.from}` : candidate.path;
  switch (candidate.kind) {
    case "appeared":
      return { title: "Appeared here", detail: "Lines first appeared at this position" };
    case "modified":
      return { title: "Changed here", detail: "Lines replaced earlier ones at this position" };
    case "moved":
      return here
        ? { title: "Moved within the file", detail: `Lines came from ${where}` }
        : { title: `Moved from ${fileName(candidate.path)}`, detail: `Removed from ${where} by the same commit` };
    case "copied":
      return here
        ? { title: "Copied within the file", detail: `Similar lines remain at ${where}` }
        : { title: `Copied from ${fileName(candidate.path)}`, detail: `Similar lines remain in ${where}` };
  }
}

/** `single origin, high likelihood`, as DeepGit words it; with rivals, the rank. */
export function likelihoodLabel(report: OriginReport, index: number): string {
  const candidate = report.candidates[index];
  if (!candidate) return "";
  const count = report.candidates.length;
  const rank =
    count === 1
      ? "single origin"
      : index === report.best
        ? `best of ${count} origins`
        : `${index + 1} of ${count} origins`;
  return `${rank}, ${candidate.likelihood} likelihood`;
}

export function deeperHint(candidate: OriginCandidate | null): string {
  if (!candidate) return "Pick a line first";
  if (!candidate.deeper) return "Nothing older: these lines were written in the first version of the file";
  return `Blame ${fileName(candidate.deeper.path)} as it was before, at line ${candidate.deeper.line}`;
}
