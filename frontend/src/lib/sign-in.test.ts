import { describe, expect, it, vi } from "vitest";
import type { SignIn } from "$lib/ipc/bindings";
import { signedIn, type SignInRequest } from "./sign-in";

function refusal(username: string | null, stderr = "fatal: could not read Username: terminal prompts disabled") {
  return {
    detail: {
      kind: "command",
      data: { stderr, stdout: "", authRequired: { host: "cdb.example", username } },
    },
  };
}

const typed = (secret: string): SignIn => ({ host: "cdb.example", username: "ann", secret, remember: true });

describe("signedIn", () => {
  it("runs once and asks nobody when the command succeeds", async () => {
    const ask = vi.fn();
    const operation = vi.fn(async () => "done");

    expect(await signedIn(operation, ask)).toBe("done");
    expect(operation).toHaveBeenCalledWith(null);
    expect(ask).not.toHaveBeenCalled();
  });

  it("asks for the refused host and runs again with the answer", async () => {
    const operation = vi.fn().mockRejectedValueOnce(refusal(null)).mockResolvedValueOnce("done");
    const ask = vi.fn(async () => typed("pw"));

    expect(await signedIn(operation, ask)).toBe("done");
    expect(ask).toHaveBeenCalledWith({ host: "cdb.example", username: null, error: null });
    expect(operation).toHaveBeenLastCalledWith(typed("pw"));
  });

  it("asks again with the username and git's words when the retry is refused too", async () => {
    const operation = vi
      .fn()
      .mockRejectedValueOnce(refusal(null))
      .mockRejectedValueOnce(refusal("ann", "fatal: Authentication failed"))
      .mockResolvedValueOnce("done");
    const asked: SignInRequest[] = [];
    const ask = vi.fn(async (request: SignInRequest) => {
      asked.push(request);
      return typed(asked.length === 1 ? "wrong" : "pw");
    });

    expect(await signedIn(operation, ask)).toBe("done");
    expect(asked[1]).toEqual({ host: "cdb.example", username: "ann", error: "fatal: Authentication failed" });
  });

  // Closing the dialog is the user's cancel, not a failure: no Errors window for it.
  it("ends as canceled when the dialog is closed", async () => {
    const operation = vi.fn().mockRejectedValue(refusal(null));

    await expect(signedIn(operation, async () => null)).rejects.toMatchObject({ detail: { kind: "cancelled" } });
    expect(operation).toHaveBeenCalledTimes(1);
  });

  it("never asks about a failure that is not about credentials", async () => {
    const error = { detail: { kind: "command", data: { stderr: "rejected", stdout: "", authRequired: null } } };
    const ask = vi.fn();

    await expect(signedIn(vi.fn().mockRejectedValue(error), ask)).rejects.toBe(error);
    expect(ask).not.toHaveBeenCalled();
  });
});
