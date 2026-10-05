import { describe, expect, it } from "vitest";
import { credentialTrouble } from "./credentials";

describe("credentialTrouble", () => {
  it("tells a closed sign-in window from no helper at all", () => {
    const canceled =
      "fatal: User cancelled dialog.\nfatal: could not read Username for 'https://github.com': terminal prompts disabled";
    expect(credentialTrouble(canceled)).toBe("canceled");
    expect(credentialTrouble("fatal: could not read Username for 'https://x': terminal prompts disabled")).toBe("noPrompt");
    expect(credentialTrouble("! [rejected] main -> main (fetch first)")).toBeNull();
  });
});
