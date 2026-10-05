import { describe, expect, it } from "vitest";
import { percentOf, progressOf, signalsFor, type TaskbarInputs } from "./taskbar";

const idle: TaskbarInputs = {
  enabled: true,
  flashEnabled: true,
  running: false,
  line: null,
  errors: 0,
  warnings: 0,
  unviewed: 0,
  flash: null,
};

describe("percentOf", () => {
  it("reads the last percentage on the line", () => {
    expect(percentOf("Receiving objects:  45% (9/20)")).toBe(45);
    expect(percentOf("remote: Compressing 100% (3/3), done. 7%")).toBe(7);
  });
  it("is null without one", () => {
    expect(percentOf(null)).toBeNull();
    expect(percentOf("From github.com:a/b")).toBeNull();
  });
  it("never passes 100", () => {
    expect(percentOf("999%")).toBe(100);
  });
});

describe("progressOf", () => {
  it("is nothing while idle, a percent when Git said one, else indeterminate", () => {
    expect(progressOf(false, "50%")).toBeNull();
    expect(progressOf(true, "Writing objects: 50%")).toEqual({ kind: "percent", value: 50 });
    expect(progressOf(true, null)).toEqual({ kind: "indeterminate" });
  });
});

describe("signalsFor", () => {
  it("passes the counts and the flash through", () => {
    const signals = signalsFor({ ...idle, running: true, errors: 1, warnings: 2, unviewed: 3, flash: "persistent" });
    expect(signals).toEqual({
      progress: { kind: "indeterminate" },
      errors: 1,
      warnings: 2,
      unviewed: 3,
      flash: "persistent",
    });
  });
  it("drops the flash alone when the flash switch is off", () => {
    const signals = signalsFor({ ...idle, flashEnabled: false, errors: 1, flash: "persistent" });
    expect(signals.flash).toBeNull();
    expect(signals.errors).toBe(1);
  });
  it("sends the empty state when the taskbar is off, so the last one is cleared", () => {
    expect(signalsFor({ ...idle, enabled: false, running: true, errors: 4, flash: "persistent" })).toEqual({});
  });
});
