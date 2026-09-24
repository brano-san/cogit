import { describe, expect, it, vi } from "vitest";

const reads = vi.hoisted(() => new Map<string, (body: string) => void>());

vi.mock("$lib/ipc", () => ({
  CogitError: class extends Error {},
  readHook: vi.fn(
    (_repo: unknown, name: string) => new Promise((resolve) => reads.set(name, resolve)),
  ),
  bypassLog: vi.fn(),
  exportPreset: vi.fn(),
  installPreset: vi.fn(),
  listPresets: vi.fn(),
  removePreset: vi.fn(),
  listHooks: vi.fn(),
  runHook: vi.fn(),
  setHookEnabled: vi.fn(),
  useHooksPath: vi.fn(),
  writeHook: vi.fn(),
}));

const { hooks } = await import("./hooks.svelte");

describe("opening one hook after another", () => {
  // Save writes `body` into `editing`: the first hook's script, arriving late, used to be
  // saved into the second.
  it("never puts one hook's script in another hook's editor", async () => {
    hooks.overview = {
      hooks: [
        { name: "pre-commit", state: "enabled" },
        { name: "pre-push", state: "enabled" },
      ],
    } as never;

    const first = hooks.edit(1 as never, "pre-commit");
    const second = hooks.edit(1 as never, "pre-push");
    reads.get("pre-push")?.("push script");
    await second;
    reads.get("pre-commit")?.("commit script");
    await first;

    expect(hooks.editing).toBe("pre-push");
    expect(hooks.body).toBe("push script");
    expect(hooks.dirty).toBe(false);
  });
});
