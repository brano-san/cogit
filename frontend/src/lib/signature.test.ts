import { describe, expect, it } from "vitest";
import { signatureLabel } from "./signature";

describe("signatureLabel", () => {
  it("names the status and the signer", () => {
    expect(signatureLabel({ status: "G", signer: "Ann <a@x>", key: "K", raw: "" })).toBe("Good signature — Ann <a@x>");
    expect(signatureLabel({ status: "E", signer: "", key: "", raw: "" })).toBe("Cannot be checked (key or gpg missing)");
  });
});
