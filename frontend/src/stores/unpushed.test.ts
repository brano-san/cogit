import { beforeEach, describe, expect, it, vi } from "vitest";

const answers = vi.hoisted(() => [] as { resolve: (oids: string[]) => void; reject: (err: unknown) => void }[]);
vi.mock("$lib/ipc", () => ({
  unpushedCommits: vi.fn(
    () => new Promise<string[]>((resolve, reject) => answers.push({ resolve, reject })),
  ),
}));

const { unpushed } = await import("./unpushed.svelte");
const A = 1 as never;

beforeEach(() => {
  answers.length = 0;
  unpushed.clear();
});

describe("the unpushed commits of the graph", () => {
  it("holds what the repository answered", async () => {
    const asked = unpushed.refresh(A);
    answers[0]!.resolve(["a1", "b2"]);
    await asked;

    expect(unpushed.has("a1")).toBe(true);
    expect(unpushed.has("c3")).toBe(false);
  });

  it("keeps the newest answer when an older one comes late", async () => {
    const first = unpushed.refresh(A);
    const second = unpushed.refresh(A);
    answers[1]!.resolve(["new"]);
    answers[0]!.resolve(["old"]);
    await Promise.all([first, second]);

    expect(unpushed.has("new")).toBe(true);
    expect(unpushed.has("old")).toBe(false);
  });

  it("counts everything as pushed when the question fails", async () => {
    const before = unpushed.refresh(A);
    answers[0]!.resolve(["a1"]);
    await before;
    const failed = unpushed.refresh(A);
    answers[1]!.reject(new Error("locked"));
    await failed;

    expect(unpushed.has("a1")).toBe(false);
  });
});
