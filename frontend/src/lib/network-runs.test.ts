import { describe, expect, it, vi } from "vitest";
import { noRemote, runGuarded, type RunHost } from "./network-runs";

function host(epoch = 1) {
  const state = { epoch };
  const fake = {
    epoch: () => state.epoch,
    report: vi.fn(),
    afterMutation: vi.fn(async () => {}),
  } satisfies RunHost;
  return { state, fake };
}

describe("runGuarded", () => {
  it("returns the starting epoch and reads nothing again on success", async () => {
    const { fake } = host(7);
    const step = vi.fn(async () => "done");

    expect(await runGuarded(fake, "Could not push", step)).toBe(7);
    expect(step).toHaveBeenCalledOnce();
    expect(fake.report).not.toHaveBeenCalled();
    expect(fake.afterMutation).not.toHaveBeenCalled();
  });

  it("reports a failure and refreshes the panels of the same repository", async () => {
    const { fake } = host();
    const err = new Error("rejected");

    expect(await runGuarded(fake, "Could not pull", () => Promise.reject(err))).toBeNull();
    expect(fake.report).toHaveBeenCalledWith(err, "Could not pull");
    expect(fake.afterMutation).toHaveBeenCalledOnce();
  });

  // Another repository came to the front while git ran: its panels are not ours to reload.
  it("only reports a failure once another repository is on screen", async () => {
    const { state, fake } = host();
    const err = new Error("timed out");

    const ran = await runGuarded(fake, "Could not fetch", async () => {
      state.epoch += 1;
      throw err;
    });

    expect(ran).toBeNull();
    expect(fake.report).toHaveBeenCalledWith(err, "Could not fetch");
    expect(fake.afterMutation).not.toHaveBeenCalled();
  });
});

describe("noRemote", () => {
  it("says what it always said, under the step's title", () => {
    expect(noRemote("push")).toEqual(["This repository has no remote.", "Could not push"]);
  });
});
