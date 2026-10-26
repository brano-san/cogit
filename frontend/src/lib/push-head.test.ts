import { describe, expect, it, vi } from "vitest";
import { requestPush } from "./push-head";
import { headPushSource, nothingPushed } from "./toolbar";

describe("Commit and Push", () => {
  it("hands the push to the main window", async () => {
    const emit = vi.fn(async () => {});

    await requestPush(3, emit);

    expect(emit).toHaveBeenCalledWith({ repo: 3 });
  });
});

describe("the detached push resolver", () => {
  it("tells an empty detached push from an ordinary one", () => {
    expect(nothingPushed({ branches: [] })).toBe(true);
    expect(nothingPushed({ branches: ["topic"] })).toBe(false);
    expect(nothingPushed(null)).toBe(false);
  });

  it("pushes HEAD itself, opening on the remote it was asked from", () => {
    expect(headPushSource()).toEqual({ kind: "head", name: "HEAD", upstream: null, remote: undefined });
    expect(headPushSource("fork").remote).toBe("fork");
  });
});
