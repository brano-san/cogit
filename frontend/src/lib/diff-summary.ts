import type { BinaryCause, BlobSide } from "./ipc";

/** `1,000,000 bytes`: sizes are exact, as SmartGit shows them (R-531). */
export function byteCount(bytes: number): string {
  return `${bytes.toLocaleString("en-US")} ${bytes === 1 ? "byte" : "bytes"}`;
}

/** In SmartGit's words: `File is considered as binary: invalid character 0x02 in line 1, at
    position 6`, and which version the character is in. */
export function binaryReason(cause: BinaryCause): string {
  if (cause.kind === "attribute") return `File is considered as binary: .gitattributes marks it ${cause.name}`;
  const code = `0x${cause.code.toString(16).padStart(2, "0").toUpperCase()}`;
  return (
    `File is considered as binary: invalid character ${code} in line ${cause.line}, ` +
    `at position ${cause.position} (${cause.side} version)`
  );
}

function kilobytes(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
}

/** The size line of an image diff. An added or deleted image says so: `3.1 KB → 0 B` read
    as a file emptied, not removed. A side is gone when it has no picture and no bytes. */
export function imageSizes(oldSize: number, newSize: number, before: string | null, after: string | null): string {
  if (after === null && newSize === 0 && oldSize > 0) return `Deleted · ${kilobytes(oldSize)}`;
  if (before === null && oldSize === 0 && newSize > 0) return `Added · ${kilobytes(newSize)}`;
  return `${kilobytes(oldSize)} → ${kilobytes(newSize)}`;
}

export function tooLargeReason(limit: number): string {
  return `File size exceeds the limit of ${byteCount(limit)}`;
}

export interface SummaryRow {
  label: "Old version" | "New version";
  /** What stands in the size column: the size, or that the file is not on this side. */
  size: string;
  /** The object id, `null` where there is none to show; `note` then says why. */
  id: string | null;
  note: string | null;
}

function row(label: SummaryRow["label"], side: BlobSide | null, missing: string): SummaryRow {
  if (!side) return { label, size: missing, id: null, note: null };
  return {
    label,
    size: byteCount(side.size),
    id: side.id,
    note: side.id === null ? "Not computed: the working file is too large to read" : null,
  };
}

/** The two versions of a file shown as a summary. An added file has no old version and a
    deleted one no new version: said so, not shown as 0 bytes. */
export function summaryRows(old: BlobSide | null, next: BlobSide | null): SummaryRow[] {
  return [row("Old version", old, "Not there: the file is added"), row("New version", next, "Not there: the file is deleted")];
}
