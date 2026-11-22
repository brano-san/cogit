import { describe, expect, it } from "vitest";
import { messageGuides } from "./commit-message";

describe("commit message guides", () => {
  it("makes both columns available only when enabled", () => {
    expect(messageGuides("Subject", false, 40, 80)).toBeNull();
    expect(messageGuides("Subject", true, 40, 80)).toEqual({
      position: "40ch 0, 80ch 0", subjectOver: false,
    });
  });

  it("warns only past the subject column and ignores the body", () => {
    expect(messageGuides("é🙂\r\nlong body", true, 2, 72)?.subjectOver).toBe(false);
    expect(messageGuides("é🙂!", true, 2, 72)?.subjectOver).toBe(true);
  });
});
