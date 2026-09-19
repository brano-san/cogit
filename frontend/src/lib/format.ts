import type { Branch, Head, Tag } from "./ipc";

const SHORT_OID = 7;

export function headLabel(head: Head | null | undefined): string {
  if (!head) return "—";
  switch (head.kind) {
    case "branch":
      return head.name;
    case "detached":
      return `detached at ${head.oid.slice(0, SHORT_OID)}`;
    case "unborn":
      return `${head.name} (unborn)`;
  }
}

/** Order comes from the backend; sorting in both places would be two sources of truth. */
export function splitBranches(branches: Branch[]): { local: Branch[]; remote: Branch[] } {
  return {
    local: branches.filter((b) => b.kind === "local"),
    remote: branches.filter((b) => b.kind === "remote"),
  };
}

export function shortOid(oid: string): string {
  return oid.slice(0, SHORT_OID);
}

/** The author's timezone, not the reader's — otherwise the output is machine-dependent. */
export function formatCommitDate(timestamp: number, offsetMinutes: number): string {
  const shifted = new Date((timestamp + offsetMinutes * 60) * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return (
    `${shifted.getUTCFullYear()}-${pad(shifted.getUTCMonth() + 1)}-${pad(shifted.getUTCDate())}` +
    ` ${pad(shifted.getUTCHours())}:${pad(shifted.getUTCMinutes())}`
  );
}

export type RefKind = "head" | "local" | "remote" | "tag";

export interface RefLabel {
  text: string;
  kind: RefKind;
}

/** The row truncates from the right, so order here is priority order. */
const REF_ORDER: Record<RefKind, number> = { head: 0, local: 1, remote: 2, tag: 3 };

export function refLabels(
  branches: Branch[],
  tags: Tag[],
  head: Head | null | undefined,
): Map<string, RefLabel[]> {
  const headBranch = head?.kind === "branch" ? head.name : null;
  const byOid = new Map<string, RefLabel[]>();

  const add = (oid: string, label: RefLabel) => {
    const existing = byOid.get(oid);
    if (existing) existing.push(label);
    else byOid.set(oid, [label]);
  };

  for (const branch of branches) {
    const kind: RefKind =
      branch.kind === "remote" ? "remote" : branch.name === headBranch ? "head" : "local";
    add(branch.oid, { text: branch.name, kind });
  }
  for (const tag of tags) {
    add(tag.oid, { text: tag.name, kind: "tag" });
  }

  for (const labels of byOid.values()) {
    labels.sort((a, b) => REF_ORDER[a.kind] - REF_ORDER[b.kind]);
  }
  return byOid;
}
