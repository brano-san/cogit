/** Patches (F-948): what kind of text a patch is, what the preview lists, what a saved
    file is called. */

export type PatchKind = "mailbox" | "diff";

/** The way a patch goes in: `git am --3way`, `git apply`, `git apply --3way` (index too). */
export type PatchMode = "mailbox" | "workTree" | "index";

const MAIL_FROM = /^From [0-9a-f]{40,64} /;
const HEADER = /^(From|Subject|Date):\s/;

/** A mailbox of `format-patch` starts with `From <sha>` or carries mail headers before its
    first diff; any other text with a diff in it is a plain diff; anything else is no patch. */
export function patchKind(text: string): PatchKind | null {
  const lines = text.replaceAll("\r\n", "\n").split("\n");
  const firstDiff = lines.findIndex((line) => line.startsWith("diff --git ") || line.startsWith("--- "));
  const hunk = lines.some((line) => line.startsWith("@@ ") || line === "GIT binary patch");
  if (firstDiff < 0 || !hunk) return null;
  const head = lines.slice(0, firstDiff);
  const first = head.find((line) => line.trim() !== "") ?? "";
  if (MAIL_FROM.test(first)) return "mailbox";
  const headers = head.filter((line) => HEADER.test(line)).length;
  return headers >= 2 && head.some((line) => line.startsWith("Subject:")) ? "mailbox" : "diff";
}

/** The subject of each mail, unfolded, without git's `[PATCH n/m]`. */
export function mailSubjects(text: string): string[] {
  const lines = text.replaceAll("\r\n", "\n").split("\n");
  const subjects: string[] = [];
  let inHeaders = true;
  for (let at = 0; at < lines.length; at += 1) {
    const line = lines[at] ?? "";
    if (MAIL_FROM.test(line)) {
      inHeaders = true;
      continue;
    }
    if (line === "") inHeaders = false;
    if (!inHeaders || !line.startsWith("Subject:")) continue;
    let subject = line.slice("Subject:".length).trim();
    while (at + 1 < lines.length && /^[ \t]/.test(lines[at + 1] ?? "")) {
      at += 1;
      subject += ` ${(lines[at] ?? "").trim()}`;
    }
    subjects.push(subject.replace(/^\[PATCH[^\]]*\]\s*/, ""));
  }
  return subjects;
}

/** The mode a patch of `kind` is applied with unless the user picks another. */
export function defaultMode(kind: PatchKind | null): PatchMode {
  return kind === "mailbox" ? "mailbox" : "workTree";
}

/** Why the mode does not fit the text, or null. A mailbox also applies as a plain diff. */
export function modeProblem(kind: PatchKind | null, mode: PatchMode): string | null {
  if (kind === null) return "This text is not a patch";
  if (mode === "mailbox" && kind !== "mailbox") return "Only a patch made by format-patch can be applied as commits";
  return null;
}

export interface FileStat {
  path: string;
  oldPath: string | null;
  added: number | null;
  removed: number | null;
}

export interface StatRow {
  label: string;
  counts: string;
  binary: boolean;
}

/** A row of the preview: `old → new` for a rename, `+a −r` or "binary". Files a mailbox
    touches several times are listed once, their counts added up. */
export function statRows(files: readonly FileStat[]): StatRow[] {
  const merged = new Map<string, FileStat>();
  for (const file of files) {
    const key = `${file.oldPath ?? ""}\0${file.path}`;
    const known = merged.get(key);
    if (!known) {
      merged.set(key, { ...file });
      continue;
    }
    known.added = known.added === null || file.added === null ? null : known.added + file.added;
    known.removed = known.removed === null || file.removed === null ? null : known.removed + file.removed;
  }
  return [...merged.values()].map((file) => {
    const binary = file.added === null || file.removed === null;
    return {
      label: file.oldPath ? `${file.oldPath} → ${file.path}` : file.path,
      counts: binary ? "binary" : `+${file.added} −${file.removed}`,
      binary,
    };
  });
}

const NAME_MAX = 64;

/** The name `git format-patch` gives the file of a commit: numbered, the subject with every
    run of other characters turned into one dash. */
export function patchFileName(subject: string, number = 1): string {
  const prefix = `${String(number).padStart(4, "0")}-`;
  const suffix = ".patch";
  const slug = subject
    .replace(/[^A-Za-z0-9._]+/g, "-")
    .replace(/\.{2,}/g, ".")
    .replace(/^[-.]+/, "")
    .slice(0, NAME_MAX - prefix.length - suffix.length)
    .replace(/[-.]+$/, "");
  return `${prefix}${slug}${suffix}`;
}
