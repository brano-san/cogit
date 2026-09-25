import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  flowStatus: vi.fn(),
  flowFinish: vi.fn(async () => {}),
  flowInit: vi.fn(async () => {}),
  flowStart: vi.fn(async () => {}),
}));
vi.mock("$lib/ipc", () => ipc);

const { flow } = await import("./flow.svelte");
const REPO = 1 as never;

const on = (head: string) => ({
  initialised: true,
  config: { main: "main", develop: "develop", feature: "feature/", release: "release/", hotfix: "hotfix/" },
  branches: ["a", "b"].map((name) => ({ kind: "feature", name, full: `feature/${name}`, isHead: name === head })),
});

beforeEach(() => {
  flow.clear();
  ipc.flowStatus.mockReset();
});

// Opened on feature/a, switched to feature/b in Branches: Finish This Branch… merged and
// deleted feature/a, the branch HEAD had left.
describe("the branch Finish acts on", () => {
  it("is the one checked out now, not the one of the last read", async () => {
    ipc.flowStatus.mockResolvedValueOnce(on("a")).mockResolvedValueOnce(on("b"));
    await flow.refresh(REPO);
    expect(flow.current?.name).toBe("a");

    const branch = await flow.headNow(REPO);

    expect(branch?.name).toBe("b");
  });

  it("is none once HEAD has left every flow branch", async () => {
    ipc.flowStatus.mockResolvedValueOnce(on("a")).mockResolvedValueOnce(on("develop"));
    await flow.refresh(REPO);

    expect(await flow.headNow(REPO)).toBeNull();
  });
});
