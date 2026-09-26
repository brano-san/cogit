import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { lookUpWhenSettled, type Named } from "./rev-lookup";

describe("lookUpWhenSettled", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("asks once the typing rests, not per key", async () => {
    const resolve = vi.fn(async (rev: string) => `commit of ${rev}`);
    const taken: Named<string>[] = [];
    const stop = lookUpWhenSettled("HE", resolve, (named) => taken.push(named), 200);
    stop();
    lookUpWhenSettled("HEAD", resolve, (named) => taken.push(named), 200);
    await vi.advanceTimersByTimeAsync(200);
    expect(resolve).toHaveBeenCalledTimes(1);
    expect(taken).toEqual([{ rev: "HEAD", commit: "commit of HEAD" }]);
  });

  it("clears the answer of an emptied field at once", () => {
    const taken: Named<string>[] = [];
    lookUpWhenSettled("  ", async () => "x", (named) => taken.push(named), 200);
    expect(taken).toEqual([null]);
  });

  // "HEA" was slow to answer, "HEAD" quick: the late "HEA" left the field with no commit.
  it("drops an answer to text typed over since", async () => {
    let answerOld: (value: string | null) => void = () => {};
    const resolve = (rev: string) =>
      rev === "HEA" ? new Promise<string | null>((done) => (answerOld = done)) : Promise.resolve("head");
    let shown: Named<string> = null;
    const take = (named: Named<string>) => (shown = named);

    const stop = lookUpWhenSettled("HEA", resolve, take, 200);
    await vi.advanceTimersByTimeAsync(200);
    stop();
    lookUpWhenSettled("HEAD", resolve, take, 200);
    await vi.advanceTimersByTimeAsync(200);
    answerOld(null);
    await vi.advanceTimersByTimeAsync(0);

    expect(shown).toEqual({ rev: "HEAD", commit: "head" });
  });
});
