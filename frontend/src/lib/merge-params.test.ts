import { describe, expect, it } from "vitest";
import { CogitError } from "./ipc";
import { failureText } from "./merge-params";

describe("failureText", () => {
  // String(err) gave only "git add -- a.txt failed with exit code 128": git's reason was lost.
  it("keeps git's own output of a command that failed", () => {
    const err = new CogitError({
      kind: "command",
      data: {
        id: 3,
        repo: "C:/work/app",
        command: "git add -- a.txt",
        exitCode: 128,
        stdout: "",
        stderr: "fatal: Unable to create 'C:/work/app/.git/index.lock': File exists.",
        operation: "Stage",
        summary: "fatal: Unable to create 'C:/work/app/.git/index.lock': File exists.",
      },
    });
    const text = failureText(err);
    expect(text).toContain("git add -- a.txt");
    expect(text).toContain("128");
    expect(text).toContain("index.lock': File exists.");
  });

  it("says what anything else said", () => {
    expect(failureText(new CogitError({ kind: "io", data: "os error 32" }))).toContain("os error 32");
  });
});
