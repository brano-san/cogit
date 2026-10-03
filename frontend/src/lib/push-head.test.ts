import { describe, expect, it, vi } from "vitest";
import { pushDetached } from "./push-head";
import { headPushSource, nothingPushed } from "./toolbar";

// Commit and Push on a detached HEAD said "HEAD is not on a branch" and pushed nothing.
describe("Commit and Push on a detached HEAD", () => {
  it("pushes by the toolbar's rule and stays put when a branch went", async () => {
    const emit = vi.fn(async () => {});
    const push = vi.fn(async () => ({ branches: ["topic"] }));

    expect(await pushDetached(3, "origin", push, emit)).toEqual({ branches: ["topic"] });

    expect(push).toHaveBeenCalledWith(3, "origin");
    expect(emit).not.toHaveBeenCalled();
  });

  it("asks the main window for Push To for HEAD when nothing qualified", async () => {
    const emit = vi.fn(async () => {});

    await pushDetached(3, "origin", async () => ({ branches: [] }), emit);

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
