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

describe("an answer for some authors", () => {
  // The table used to be copied per answer: every avatar on screen read the new table and
  // drew again. A reactive map wakes only the authors that came.
  it("redraws only the avatars of those authors", async () => {
    const { mergeRows } = await import("$lib/avatars");
    const emails = Array.from({ length: 30 }, (_, i) => `a${i}@example.com`);
    const row = (email: string) => ({ email, initials: "XX", color: "#000", image: null });
    mergeRows(avatars.rows, emails.map(row));
    const runs = new Map<string, number>();
    const stop = $effect.root(() => {
      for (const email of emails) {
        $effect(() => {
          void avatars.look(email);
          runs.set(email, (runs.get(email) ?? 0) + 1);
        });
      }
    });
    flushSync();
    runs.clear();
    mergeRows(avatars.rows, [{ ...row("a3@example.com"), image: "data:image/png;base64,AA" }]);
    flushSync();
    expect([...runs.keys()]).toEqual(["a3@example.com"]);
    stop();
    avatars.rows.clear();
  });
});
