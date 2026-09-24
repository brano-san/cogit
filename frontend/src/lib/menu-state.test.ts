import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MENU_STATE_DELAY_MS, menuStatePusher } from "./menu-state";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function pusher() {
  const send = vi.fn((_disabled: string[], _checked: string[]) => Promise.resolve());
  return { send, push: menuStatePusher(send) };
}

describe("menuStatePusher", () => {
  // One action re-ran the effect six times and more, each a call that walks every item.
  it("sends the changes of one action as one call with the last state", async () => {
    const { send, push } = pusher();
    push(["fetch"], []);
    push(["fetch", "pull"], []);
    push(["fetch", "pull"], ["panel-graph"]);
    push(["pull"], ["panel-graph"]);
    push(["pull"], ["panel-graph", "output"]);
    push(["pull"], ["panel-graph", "output"]);
    expect(send).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);

    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith(["pull"], ["panel-graph", "output"]);
  });

  it("does not send a state the menu already has", async () => {
    const { send, push } = pusher();
    push(["fetch"], ["output"]);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);
    push(["fetch"], ["output"]);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);

    expect(send).toHaveBeenCalledTimes(1);
  });

  it("treats the same ids in another order as the same state", async () => {
    const { send, push } = pusher();
    push(["a", "b"], ["c", "d"]);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);
    push(["b", "a"], ["d", "c"]);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);

    expect(send).toHaveBeenCalledTimes(1);
  });

  // A rebuilt bar starts with every tick cleared, whatever was sent before.
  it("sends the same state again when told the menu was rebuilt", async () => {
    const { send, push } = pusher();
    push(["fetch"], ["output"]);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);
    push(["fetch"], ["output"], { rebuilt: true });
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);

    expect(send).toHaveBeenCalledTimes(2);
  });

  it("tries again after a failed send", async () => {
    const { send, push } = pusher();
    send.mockRejectedValueOnce(new Error("no window"));
    push(["fetch"], []);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);
    push(["fetch"], []);
    await vi.advanceTimersByTimeAsync(MENU_STATE_DELAY_MS);

    expect(send).toHaveBeenCalledTimes(2);
  });
});
