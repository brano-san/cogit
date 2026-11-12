import { describe, expect, it } from "vitest";
import { latestOnly } from "./latest-only";

describe("latestOnly", () => {
  it("sends one at a time and skips what a newer value replaced meanwhile", async () => {
    const sent: string[] = [];
    let finish: () => void = () => {};
    const send = latestOnly<string>(
      (value) =>
        new Promise<void>((resolve) => {
          sent.push(value);
          finish = resolve;
        }),
      () => {},
    );

    send("running");
    send("idle-1");
    send("idle");
    expect(sent).toEqual(["running"]);

    finish();
    await Promise.resolve();
    await Promise.resolve();
    expect(sent).toEqual(["running", "idle"]);
  });

  it("reports a failed send", async () => {
    const failed: string[] = [];
    const send = latestOnly<string>(() => Promise.reject(new Error("no taskbar")), (value) => failed.push(value));
    send("running");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(failed).toEqual(["running"]);
  });

  it("lets a newer value keep what the one it replaced carried", async () => {
    const sent: { flash: boolean }[] = [];
    let finish: () => void = () => {};
    const send = latestOnly<{ flash: boolean }>(
      (value) =>
        new Promise<void>((resolve) => {
          sent.push(value);
          finish = resolve;
        }),
      () => {},
      (older, newer) => ({ flash: newer.flash || older.flash }),
    );

    send({ flash: false });
    send({ flash: true });
    send({ flash: false });
    finish();
    await Promise.resolve();
    await Promise.resolve();

    expect(sent).toEqual([{ flash: false }, { flash: true }]);
  });
});
