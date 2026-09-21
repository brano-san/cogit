export type LineKind = "error" | "warning" | "plain";

export interface HighlightedLine {
  text: string;
  kind: LineKind;
}

/** `remote:` may be repeated, so the prefix is stripped until nothing is left to strip. */
function body(line: string): string {
  let rest = line.trimStart();
  while (rest.toLowerCase().startsWith("remote:")) {
    rest = rest.slice("remote:".length).trimStart();
  }
  return rest;
}

/** Only a leading marker counts. A line that merely contains "error" is prose. */
export function classifyLine(line: string): LineKind {
  const rest = body(line).toLowerCase();
  if (rest.startsWith("error:") || rest.startsWith("fatal:") || rest.startsWith("conflict")) {
    return "error";
  }
  if (rest.startsWith("warning:") || rest.startsWith("hint:")) return "warning";
  return "plain";
}

export function highlightStream(text: string): HighlightedLine[] {
  const lines = text.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n");
  if (lines.length === 1 && lines[0] === "") return [];
  return lines.map((line) => ({ text: line, kind: classifyLine(line) }));
}
