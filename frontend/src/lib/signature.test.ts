import { describe, expect, it } from "vitest";
import { signatureBadge, signatureLabel } from "./signature";

describe("signatureLabel", () => {
  it("names the status and the signer", () => {
    expect(signatureLabel({ status: "G", signer: "Ann <a@x>" })).toBe("Good signature — Ann <a@x>");
    expect(signatureLabel({ status: "E", signer: "" })).toBe("Cannot be checked (key, gpg or allowed signers missing)");
  });
});

describe("signatureBadge", () => {
  it("marks only signed commits", () => {
    expect(signatureBadge(undefined)).toBeNull();
    expect(signatureBadge("N")).toBeNull();
  });

  it("tells a verified signature from an unchecked and a bad one", () => {
    expect(signatureBadge("G")).toBe("verified");
    for (const status of ["U", "X", "Y", "E"]) expect(signatureBadge(status)).toBe("unverified");
    expect(signatureBadge("B")).toBe("bad");
    expect(signatureBadge("R")).toBe("bad");
  });
});
