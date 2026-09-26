import { describe, expect, it, vi } from "vitest";
import { autostashQuestion, keptNotice, runCheckout, type CheckoutSteps } from "./checkout-flow";
import { CogitError, type AutostashOutcome } from "./ipc";
import type { CheckoutRequest } from "./ref-checkout";

const request = (over: Partial<CheckoutRequest> = {}): CheckoutRequest => ({
  target: { kind: "branch", name: "topic" },
  branch: "topic",
  what: "topic",
  ...over,
});

function refused(stderr: string): CogitError {
  return new CogitError({
    kind: "command",
    data: {
      id: 1,
      repo: "r",
      command: "git switch topic",
      exitCode: 1,
      stdout: "",
      stderr,
      operation: "switch",
      summary: "",
    },
  });
}

const IN_THE_WAY = refused(
  "error: Your local changes to the following files would be overwritten by checkout:\n\ta.txt\nAborting\n",
);

function steps(over: Partial<CheckoutSteps> = {}) {
  const order: string[] = [];
  const all: CheckoutSteps = {
    elsewhere: vi.fn(async () => false),
    checkout: vi.fn(async () => void order.push("checkout")),
    ask: vi.fn(async () => ({ drop: true })),
    autostash: vi.fn(async (): Promise<AutostashOutcome> => {
      order.push("autostash");
      return { kind: "restored" };
    }),
    report: vi.fn((_err: unknown, title: string) => void order.push(`report: ${title}`)),
    inform: vi.fn((title: string) => void order.push(`inform: ${title}`)),
    after: vi.fn(async () => void order.push("after")),
    ...over,
  };
  return { all, order };
}

describe("carrying out the Checkout dialog's choice", () => {
  it("checks out and reads back", async () => {
    const { all, order } = steps();

    expect(await runCheckout(request(), all)).toBe("done");

    expect(order).toEqual(["checkout", "after"]);
    expect(all.checkout).toHaveBeenCalledWith({ kind: "branch", name: "topic" });
  });

  it("leaves a branch another worktree has to the question about that worktree", async () => {
    const { all, order } = steps({ elsewhere: vi.fn(async () => true) });

    expect(await runCheckout(request(), all)).toBe("elsewhere");

    expect(order).toEqual([]);
    expect(all.elsewhere).toHaveBeenCalledWith("topic");
  });

  it("asks nothing about worktrees for a new branch or a detached HEAD", async () => {
    const { all } = steps();

    await runCheckout(request({ target: { kind: "commit", oid: "a".repeat(40) }, branch: null }), all);

    expect(all.elsewhere).not.toHaveBeenCalled();
  });

  it("reports a refusal that a stash would not clear, and asks nothing", async () => {
    const { all, order } = steps({
      checkout: vi.fn(async () => {
        throw refused("fatal: invalid reference: topic\n");
      }),
    });

    expect(await runCheckout(request(), all)).toBe("failed");

    expect(order).toEqual(["report: Could not check out"]);
    expect(all.ask).not.toHaveBeenCalled();
  });
});

// F-132: local changes in the way — stash, check out, put them back. The steps, their
// order and what a refusal at each leaves are the backend's one operation (tests/autostash.rs).
describe("checking out with the changes stashed out of the way", () => {
  it("asks, naming what is in the way, then runs the checkout as one step", async () => {
    const { all, order } = steps({
      checkout: vi.fn(async () => {
        throw IN_THE_WAY;
      }),
    });

    expect(await runCheckout(request(), all)).toBe("done");

    expect(order).toEqual(["autostash", "after"]);
    expect(all.ask).toHaveBeenCalledWith(expect.stringContaining("a.txt"));
    expect(all.autostash).toHaveBeenCalledWith(
      { kind: "branch", name: "topic" },
      "cogit: autostash before checking out topic",
      true,
    );
  });

  // Item 46: "drop the stash once it applies cleanly", unticked, keeps it in the list.
  it("passes the dialog's drop choice on, and says where a kept stash is", async () => {
    const { all, order } = steps({
      checkout: vi.fn(async () => {
        throw IN_THE_WAY;
      }),
      ask: vi.fn(async () => ({ drop: false })),
      autostash: vi.fn(async (): Promise<AutostashOutcome> => ({ kind: "kept", clean: true })),
    });

    await runCheckout(request(), all);

    expect(all.autostash).toHaveBeenCalledWith(expect.anything(), expect.any(String), false);
    expect(order).toEqual(["inform: Changes carried over", "after"]);
  });

  it("says the changes did not apply cleanly and are still in the list", async () => {
    const { all, order } = steps({
      checkout: vi.fn(async () => {
        throw IN_THE_WAY;
      }),
      autostash: vi.fn(async (): Promise<AutostashOutcome> => ({ kind: "kept", clean: false })),
    });

    await runCheckout(request(), all);

    expect(order).toEqual(["inform: Changes did not apply cleanly", "after"]);
  });

  it("stashes for any choice: a new branch too", async () => {
    const target = { kind: "newBranch" as const, name: "review", start: "refs/remotes/origin/review", track: true };
    const { all } = steps({
      checkout: vi.fn(async () => {
        throw IN_THE_WAY;
      }),
    });

    await runCheckout(request({ target, branch: null, what: "review" }), all);

    expect(all.autostash).toHaveBeenCalledWith(target, "cogit: autostash before checking out review", true);
  });

  it("changes nothing when the user declines, and reports git's refusal", async () => {
    const { all, order } = steps({
      checkout: vi.fn(async () => {
        throw IN_THE_WAY;
      }),
      ask: vi.fn(async () => null),
    });

    expect(await runCheckout(request(), all)).toBe("declined");

    expect(order).toEqual(["report: Could not check out"]);
  });

  it("reports what the stashed checkout refused, and reads back: the tree may have changed", async () => {
    const { all, order } = steps({
      checkout: vi.fn(async () => {
        throw IN_THE_WAY;
      }),
      autostash: vi.fn(async () => {
        throw new Error("checkout refused");
      }),
    });

    expect(await runCheckout(request(), all)).toBe("done");

    expect(order).toEqual(["report: Could not check out", "after"]);
  });
});

describe("keptNotice", () => {
  it("says nothing when the stash is gone", () => {
    expect(keptNotice("topic", { kind: "restored" })).toBeNull();
  });

  it("names where the stash that stayed is", () => {
    expect(keptNotice("topic", { kind: "kept", clean: false })?.body).toContain("stash@{0}");
  });
});

describe("autostashQuestion", () => {
  it("names at most five of the files git named", () => {
    const question = autostashQuestion("topic", ["a", "b", "c", "d", "e", "f"]);
    expect(question).toBe("6 file(s) are in the way: a, b, c, d, e. Stash them, check out topic, then put them back?");
  });

  it("still asks when git named none", () => {
    expect(autostashQuestion("topic", [])).toBe("Local changes are in the way. Stash them, check out topic, then put them back?");
  });
});
