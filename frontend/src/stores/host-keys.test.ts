import { beforeEach, describe, expect, it, vi } from "vitest";

const answered = vi.hoisted(() => [] as [number, boolean][]);

vi.mock("$lib/ipc", () => ({
  answerHostKey: vi.fn(async (id: number, accept: boolean) => {
    answered.push([id, accept]);
  }),
  hostKeyQuestions: vi.fn(async () => []),
  onHostKeyAsked: vi.fn(async () => () => {}),
  onHostKeyWithdrawn: vi.fn(async () => () => {}),
}));

const { hostKeys, queued, withdrawn } = await import("./host-keys.svelte");

function ask(id: number, host = "h.example") {
  return {
    id,
    question: {
      host,
      address: null,
      keyType: "ED25519",
      fingerprint: "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU",
      otherTypesKnown: false,
    },
  };
}

describe("host key questions", () => {
  beforeEach(() => {
    answered.length = 0;
    hostKeys.asks = [];
  });

  it("queues each question once, in the order asked", () => {
    const asks = queued(queued(queued([], ask(1)), ask(2)), ask(1));
    expect(asks.map((a) => a.id)).toEqual([1, 2]);
    expect(withdrawn(asks, 1).map((a) => a.id)).toEqual([2]);
  });

  it("shows the oldest question and answers it once", () => {
    hostKeys.add(ask(1, "a.example"));
    hostKeys.add(ask(2, "b.example"));
    expect(hostKeys.current?.question.host).toBe("a.example");

    hostKeys.answer(1, true);
    hostKeys.answer(1, false);
    expect(answered).toEqual([[1, true]]);
    expect(hostKeys.current?.question.host).toBe("b.example");
  });

  it("drops a question the backend withdrew without answering it", () => {
    hostKeys.add(ask(3));
    hostKeys.withdraw(3);
    hostKeys.answer(3, true);
    expect(hostKeys.current).toBeNull();
    expect(answered).toEqual([]);
  });
});
