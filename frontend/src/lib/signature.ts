import type { SignatureCheck } from "$lib/ipc";

const STATUS: Record<string, string> = {
  G: "Good signature",
  B: "Bad signature",
  U: "Good signature, key not trusted",
  X: "Good signature, now expired",
  Y: "Good signature, key expired",
  R: "Good signature, key revoked",
  E: "Cannot be checked (key or gpg missing)",
  N: "Not signed",
};

/** git's `%G?` letter in words, with who signed it when git knows. */
export function signatureLabel(check: SignatureCheck): string {
  const label = STATUS[check.status] ?? `Unknown (${check.status})`;
  return check.signer ? `${label} — ${check.signer}` : label;
}
