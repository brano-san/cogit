import { describe, expect, it, vi } from "vitest";
import { flushSync } from "svelte";

const commands = { setAvatars: vi.fn(), avatarWindow: vi.fn(), avatarsFor: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { avatars } = await import("./avatars.svelte");

describe("avatars.apply called from an effect", () => {
  // App's effect read `enabled` through `apply`; the failure turning it back off re-ran the
  // effect, which asked again, for ever, when the cache directory could not be made.
  it("asks once when turning them on fails", async () => {
    // Past a few calls it stops answering: the loop ran on microtasks and starved the test.
    commands.setAvatars.mockImplementation(() =>
      commands.setAvatars.mock.calls.length > 3
        ? new Promise(() => {})
        : Promise.resolve({ status: "error", error: { kind: "io", data: "denied" } }),
    );
    const stop = $effect.root(() => {
      $effect(() => void avatars.apply(true));
    });
    for (let i = 0; i < 5; i++) {
      flushSync();
      await Promise.resolve();
    }
    expect(commands.setAvatars).toHaveBeenCalledTimes(1);
    expect(avatars.enabled).toBe(false);
    stop();
  });
});
