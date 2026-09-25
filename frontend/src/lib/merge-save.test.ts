import { describe, expect, it, vi } from "vitest";
import { answerMergeResolved, saveResolution, type SaveSteps } from "./merge-save";

function steps(over: Partial<SaveSteps> = {}): { steps: SaveSteps; order: string[] } {
  const order: string[] = [];
  const step = (name: string) => vi.fn(async () => void order.push(name));
  return {
    order,
    steps: {
      resolve: step("resolve"),
      announce: step("announce"),
      saved: vi.fn(() => void order.push("saved")),
      close: step("close"),
      ...over,
    },
  };
}

// F-229: Save in the merge window writes the file, tells the main window, and closes; the
// main window closes its panel and reads the tree again.
describe("Save in the merge window", () => {
  it("writes the resolution, announces it, then closes the window behind itself", async () => {
    const { steps: save, order } = steps();

    expect(await saveResolution(save)).toBeNull();

    expect(order).toEqual(["resolve", "announce", "saved", "close"]);
  });

  it("announces nothing and stays open when Git refuses the write", async () => {
    const { steps: save, order } = steps({
      resolve: vi.fn(async () => {
        throw new Error("index.lock exists");
      }),
    });

    expect(await saveResolution(save)).toContain("index.lock exists");

    expect(order).toEqual([]);
  });
});

describe("the main window hearing of it", () => {
  it("closes that file's panel and reads the working tree again", async () => {
    const resolvedElsewhere = vi.fn();
    const reload = vi.fn(async () => {});

    await answerMergeResolved({ repo: 1, path: "a.txt" }, { shown: () => 1, resolvedElsewhere, reload });

    expect(resolvedElsewhere).toHaveBeenCalledWith("a.txt");
    expect(reload).toHaveBeenCalledOnce();
  });

  it("leaves the panels alone when they show another repository", async () => {
    const resolvedElsewhere = vi.fn();
    const reload = vi.fn(async () => {});

    await answerMergeResolved({ repo: 1, path: "a.txt" }, { shown: () => 2, resolvedElsewhere, reload });
    await answerMergeResolved({ repo: 1, path: "a.txt" }, { shown: () => null, resolvedElsewhere, reload });

    expect(resolvedElsewhere).not.toHaveBeenCalled();
    expect(reload).not.toHaveBeenCalled();
  });
});
