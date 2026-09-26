import { describe, expect, it } from "vitest";
import { ShownRepository } from "./shown-repository";

function deferredSender() {
  const sent: (number | null)[] = [];
  const pending: (() => void)[] = [];
  const send = (repo: number | null) =>
    new Promise<void>((resolve) => {
      sent.push(repo);
      pending.push(resolve);
    });
  const answer = async () => {
    pending.shift()?.();
    await Promise.resolve();
    await Promise.resolve();
  };
  return { sent, send, answer };
}

describe("ShownRepository", () => {
  it("tells the backend the repository shown", () => {
    const { sent, send } = deferredSender();
    new ShownRepository(send).set(3);
    expect(sent).toEqual([3]);
  });

  // Two calls in flight can land in either order; the older one landing last would watch
  // the repository just left.
  it("waits for the call in flight and then sends only the latest wish", async () => {
    const { sent, send, answer } = deferredSender();
    const shown = new ShownRepository(send);

    shown.set(1);
    shown.set(2);
    shown.set(3);
    expect(sent).toEqual([1]);

    await answer();
    expect(sent).toEqual([1, 3]);
    await answer();
    expect(sent).toEqual([1, 3]);
  });

  it("does not repeat a repository already sent", async () => {
    const { sent, send, answer } = deferredSender();
    const shown = new ShownRepository(send);

    shown.set(4);
    await answer();
    shown.set(4);

    expect(sent).toEqual([4]);
  });

  it("sends nothing shown once the panels are empty", async () => {
    const { sent, send, answer } = deferredSender();
    const shown = new ShownRepository(send);

    shown.set(4);
    await answer();
    shown.set(null);

    expect(sent).toEqual([4, null]);
  });

  it("keeps going after a call that failed", async () => {
    const sent: (number | null)[] = [];
    const shown = new ShownRepository((repo) => {
      sent.push(repo);
      return repo === 1 ? Promise.reject(new Error("gone")) : Promise.resolve();
    });

    shown.set(1);
    shown.set(2);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(sent).toEqual([1, 2]);
  });
});
