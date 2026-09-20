import { describe, expect, it } from "vitest";
import { REPORT_FLOOR_MS, timer } from "./timing";

function recorder() {
  const seen: { label: string; ms: number; detail: string }[] = [];
  return { seen, sink: (label: string, ms: number, detail: string) => seen.push({ label, ms, detail }) };
}

function clock(values: number[]) {
  let at = 0;
  return () => values[Math.min(at++, values.length - 1)] ?? 0;
}

describe("timer", () => {
  it("reports an interval the user would notice", () => {
    const { seen, sink } = recorder();
    timer("select-commit", sink, clock([0, 400])).stop();
    expect(seen).toEqual([{ label: "select-commit", ms: 400, detail: "" }]);
  });

  it("stays quiet below the floor, so a fast app writes nothing", () => {
    const { seen, sink } = recorder();
    timer("select-commit", sink, clock([0, REPORT_FLOOR_MS - 1])).stop();
    expect(seen).toEqual([]);
  });

  it("rounds to whole milliseconds", () => {
    const { seen, sink } = recorder();
    timer("diff", sink, clock([10.2, 260.9])).stop();
    expect(seen[0]?.ms).toBe(251);
  });

  it("carries the detail that explains the number", () => {
    const { seen, sink } = recorder();
    timer("diff", sink, clock([0, 500])).stop("1200 rows");
    expect(seen[0]?.detail).toBe("1200 rows");
  });

  it("reports once however often it is stopped", () => {
    const { seen, sink } = recorder();
    const running = timer("diff", sink, clock([0, 500, 900]));
    running.stop();
    running.stop();
    expect(seen).toHaveLength(1);
  });

  it("never throws when the sink does", () => {
    const angry = () => {
      throw new Error("IPC is gone");
    };
    expect(() => timer("diff", angry, clock([0, 500])).stop()).not.toThrow();
  });
});
