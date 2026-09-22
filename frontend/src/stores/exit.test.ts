import { describe, expect, it } from "vitest";
import type { Operation, OperationChanged, RepoId } from "$lib/ipc";
import { ExitFlow } from "./exit.svelte";

const repo = 1 as unknown as RepoId;

function op(id: number, phase: Operation["phase"] = "running"): Operation {
  return { id, repo, kind: "push", label: "Pushing", phase, success: null };
}

function done(id: number, success = true): OperationChanged {
  return { id, repo, kind: "push", label: "Pushing", phase: "done", success };
}

const snapshot = (operations: Operation[]) => async () => operations;
const flushed = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Wrapped, because awaiting the answer itself would wait for the user. */
async function opened(flow: ExitFlow, operations: Operation[], confirmExit = true) {
  const answer = flow.ask("window", confirmExit, snapshot(operations));
  await flushed();
  return { answer };
}

describe("asking", () => {
  it("lets the window go unasked when the user turned the question off and nothing runs", async () => {
    const flow = new ExitFlow();
    await expect(flow.ask("window", false, snapshot([]))).resolves.toBe(true);
    expect(flow.prompt).toBeNull();
  });

  it("opens the question and waits for an answer", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, []);
    expect(flow.prompt).toEqual({ source: "window" });
    expect(flow.variant).toBe("plain");
    flow.answer("exit", false, true);
    await expect(answer).resolves.toBe(true);
    expect(flow.prompt).toBeNull();
  });

  it("does not stack a second question on a second click of the cross", async () => {
    const flow = new ExitFlow();
    await opened(flow, []);
    await expect(flow.ask("window", true, snapshot([]))).resolves.toBe(false);
    expect(flow.prompt).toEqual({ source: "window" });
  });

  it("treats a snapshot that failed as an empty queue", async () => {
    const flow = new ExitFlow();
    const answer = flow.ask("window", true, async () => {
      throw new Error("backend gone");
    });
    await flushed();
    expect(flow.variant).toBe("plain");
    flow.answer("cancel", false, true);
    await expect(answer).resolves.toBe(false);
  });

  it("is not wedged shut by a snapshot that throws before it starts", async () => {
    const flow = new ExitFlow();
    const failing = () => {
      throw new Error("not even a promise");
    };
    const answer = flow.ask("window", true, failing);
    await flushed();
    flow.answer("cancel", false, true);
    await expect(answer).resolves.toBe(false);
    await expect(flow.ask("window", false, snapshot([]))).resolves.toBe(true);
  });

  it("replays what happened while the snapshot was on its way", async () => {
    const flow = new ExitFlow();
    let release: (operations: Operation[]) => void = () => {};
    void flow.ask("window", true, () => new Promise((resolve) => (release = resolve)));
    flow.observe(done(1));
    release([op(1)]);
    await flushed();
    expect(flow.variant).toBe("plain");
  });
});

describe("the source of the exit", () => {
  it("is the window unless the menu said otherwise just before", () => {
    const flow = new ExitFlow();
    expect(flow.takeSource()).toBe("window");
    flow.fromCommand();
    expect(flow.takeSource()).toBe("command");
    expect(flow.takeSource()).toBe("window");
  });
});

describe("while the dialog is open", () => {
  it("drops to the plain question once the last operation finishes", async () => {
    const flow = new ExitFlow();
    await opened(flow, [op(1)]);
    expect(flow.variant).toBe("busy");
    flow.observe(done(1));
    expect(flow.variant).toBe("plain");
    expect(flow.prompt).not.toBeNull();
  });

  it("lets a held shutdown go on by itself once the queue is empty", async () => {
    const flow = new ExitFlow();
    const answer = flow.ask("system", true, snapshot([op(1)]));
    await flushed();
    expect(flow.variant).toBe("busy");
    flow.observe(done(1));
    await expect(answer).resolves.toBe(true);
    expect(flow.prompt).toBeNull();
  });

  it("turns into the warning when something starts", async () => {
    const flow = new ExitFlow();
    await opened(flow, []);
    flow.observe({ ...done(7), phase: "queued", success: null });
    expect(flow.variant).toBe("busy");
  });
});

describe("answers", () => {
  it("hands back the setting to store only when the exit is confirmed", async () => {
    const flow = new ExitFlow();
    await opened(flow, []);
    expect(flow.answer("exit", true, true)).toBe(false);
  });

  it("stores nothing on Cancel, even with the box ticked", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, []);
    expect(flow.answer("cancel", true, true)).toBeNull();
    await expect(answer).resolves.toBe(false);
  });

  it("exits at once on Exit Anyway", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, [op(1)]);
    expect(flow.answer("exitAnyway", true, true)).toBeNull();
    await expect(answer).resolves.toBe(true);
  });
});

describe("Exit When Done", () => {
  it("waits for the queue to drain and then exits", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, [op(1), op(2, "queued")]);
    let settled: boolean | null = null;
    void answer.then((go) => (settled = go));

    flow.answer("exitWhenDone", false, true);
    expect(flow.waiting).toBe(true);
    flow.observe(done(1));
    await flushed();
    expect(settled).toBeNull();

    flow.observe(done(2));
    await flushed();
    expect(settled).toBe(true);
    expect(flow.waiting).toBe(false);
  });

  it("exits straight away when the queue drained before the click", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, [op(1)]);
    flow.observe(done(1));
    flow.answer("exitWhenDone", false, true);
    await expect(answer).resolves.toBe(true);
  });

  it("stays open when an operation fails, so its error can be read", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, [op(1), op(2, "queued")]);
    flow.answer("exitWhenDone", false, true);
    flow.observe(done(1, false));
    await expect(answer).resolves.toBe(false);
    expect(flow.prompt).toBeNull();
  });

  it("can be called off with Cancel", async () => {
    const flow = new ExitFlow();
    const { answer } = await opened(flow, [op(1)]);
    flow.answer("exitWhenDone", false, true);
    flow.answer("cancel", false, true);
    await expect(answer).resolves.toBe(false);
    expect(flow.waiting).toBe(false);
  });
});
