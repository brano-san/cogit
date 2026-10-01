import { describe, expect, it, vi } from "vitest";
import type { RefDeletion } from "$lib/ipc";
import { runDeletion, type DeleteHost } from "./ref-delete-run";

const request: RefDeletion = { kind: "branch", remote: null, names: ["a", "b", "c"], force: false };
const report = (over = {}) => ({ deleted: [], notFullyMerged: [], failed: [], skipped: [], ...over }) as never;

function host(reports: unknown[], force = true) {
  const remove = vi.fn(async () => reports.shift() as never);
  const fail = vi.fn();
  const askForce = vi.fn(async () => force);
  return { remove, fail, askForce, host: { remove, fail, askForce } as DeleteHost };
}

describe("runDeletion", () => {
  it("counts what went and reports each failure on its own", async () => {
    const h = host([report({ deleted: ["a"], failed: [{ name: "b", error: "boom" }] })]);
    const result = await runDeletion(request, h.host);
    expect(result).toEqual({ deleted: 1, failed: ["b"] });
    expect(h.fail).toHaveBeenCalledWith("boom", "Could not delete b");
  });

  it("offers a forced second pass for the branches not fully merged", async () => {
    const h = host([report({ deleted: ["a"], notFullyMerged: ["c"] }), report({ deleted: ["c"] })]);
    const result = await runDeletion(request, h.host);
    expect(result.deleted).toBe(2);
    expect(h.askForce).toHaveBeenCalledWith(["c"]);
    expect(h.remove).toHaveBeenLastCalledWith({ ...request, names: ["c"], force: true });
  });

  it("leaves them when the force is declined", async () => {
    const h = host([report({ notFullyMerged: ["c"] })], false);
    await runDeletion(request, h.host);
    expect(h.remove).toHaveBeenCalledTimes(1);
  });

  it("reports a call that threw and goes on", async () => {
    const remove = vi.fn(async () => {
      throw new Error("down");
    });
    const fail = vi.fn();
    const result = await runDeletion(request, { remove, fail, askForce: async () => false });
    expect(result).toEqual({ deleted: 0, failed: [] });
    expect(fail).toHaveBeenCalledWith(expect.any(Error), "Could not delete the refs");
  });
});
