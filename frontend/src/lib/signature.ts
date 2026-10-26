import type { SignatureCheck } from "$lib/ipc";

const STATUS: Record<string, string> = {
  G: "Good signature",
  B: "Bad signature",
  U: "Good signature, key not trusted",
  X: "Good signature, now expired",
  Y: "Good signature, key expired",
  R: "Good signature, key revoked",
  E: "Cannot be checked (key, gpg or allowed signers missing)",
  N: "Not signed",
};

/** git's `%G?` letter in words, with who signed it when git knows. */
export function signatureLabel(check: Pick<SignatureCheck, "status" | "signer">): string {
  const label = STATUS[check.status] ?? `Unknown (${check.status})`;
  return check.signer ? `${label} — ${check.signer}` : label;
}

export type SignatureBadge = "verified" | "unverified" | "bad";

export function signatureBadge(status: string | undefined): SignatureBadge | null {
  if (status === undefined || status === "N") return null;
  if (status === "G") return "verified";
  if (status === "B" || status === "R") return "bad";
  return "unverified";
}
