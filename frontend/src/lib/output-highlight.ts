export type LineKind = "error" | "warning" | "omitted" | "label" | "plain";

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

const ERROR_PREFIXES = ["error:", "fatal:", "conflict", "failed", "panicked at"];

/** Only a leading marker counts. A line that merely contains "error" is prose. */
export function classifyLine(line: string): LineKind {
  const rest = body(line).toLowerCase();
  if (rest.startsWith("…") && rest.endsWith("omitted, see log …")) return "omitted";
  if (ERROR_PREFIXES.some((prefix) => rest.startsWith(prefix))) return "error";
  // A panic line starts with the thread that raised it, and it is the line the reader
  // came for; nothing else in a test log matches this shape.
  if (rest.startsWith("thread ") && rest.includes("panicked at")) return "error";
  if (rest.startsWith("warning:") || rest.startsWith("hint:")) return "warning";
  return "plain";
}

export function highlightStream(text: string): HighlightedLine[] {
  const lines = text.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n");
  if (lines.length === 1 && lines[0] === "") return [];
  return lines.map((line) => ({ text: line, kind: classifyLine(line) }));
}

/** One flat list, because it is read, searched and selected as one block of text. */
export function logLines(stdout: string, stderr: string): HighlightedLine[] {
  const parts: HighlightedLine[] = [];
  const both = stdout.trim() !== "" && stderr.trim() !== "";
  for (const [label, text] of [
    ["stderr", stderr],
    ["stdout", stdout],
  ] as const) {
    if (text.trim() === "") continue;
    if (both) parts.push({ text: label, kind: "label" });
    parts.push(...highlightStream(text));
  }
  return parts;
}

/** Plain text, not a pattern: a log is full of brackets and dots nobody means as syntax. */
export function findMatches(lines: readonly HighlightedLine[], needle: string): number[] {
  const wanted = needle.toLowerCase();
  if (wanted === "") return [];
  const hits: number[] = [];
  lines.forEach((line, index) => {
    if (line.text.toLowerCase().includes(wanted)) hits.push(index);
  });
  return hits;
}
