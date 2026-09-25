import { describe, expect, it, vi } from "vitest";

const reads = vi.hoisted(() => new Map<string, (body: string) => void>());

vi.mock("$lib/ipc", () => ({
  CogitError: class extends Error {},
  toCogitError: (err: unknown) => err,
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

// One failed write left `error` set for the rest of the session; with every change to the
// notification queue it was offered again, under "Could not read the hooks" although it
// was a write that failed.
describe("an action after one that failed", () => {
  it("clears the old error and names what failed", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.listHooks).mockResolvedValue({ hooks: [] } as never);
    vi.mocked(ipc.bypassLog).mockResolvedValue([]);
    vi.mocked(ipc.listPresets).mockResolvedValue([]);
    vi.mocked(ipc.writeHook).mockRejectedValueOnce(new Error("locked"));
    hooks.editing = "pre-commit";
    hooks.body = "#!/bin/sh\n";

    await hooks.save(1 as never);
    expect(hooks.error).not.toBeNull();
    expect(hooks.failure).toBe("Could not save the hook");

    vi.mocked(ipc.writeHook).mockResolvedValueOnce(undefined as never);
    await hooks.save(1 as never);
    expect(hooks.error).toBeNull();
  });
});

// A hook the antivirus held opened as an empty editor, and Save then replaced the real
// hook with whatever was typed there.
describe("a hook that cannot be read", () => {
  it("is reported and never opened as an empty script", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.readHook).mockRejectedValueOnce(new Error("sharing violation"));
    vi.mocked(ipc.writeHook).mockClear();
    hooks.overview = { hooks: [{ name: "pre-commit", state: "enabled" }] } as never;

    await hooks.edit(1 as never, "pre-commit");

    expect(hooks.editing).toBeNull();
    expect(hooks.failure).toBe("Could not read the hook");
    await hooks.save(1 as never);
    expect(ipc.writeHook).not.toHaveBeenCalled();
  });
});

// Until the second hook's script arrived, the editor showed the first one's under the
// second one's name, and Save wrote it there.
describe("a hook still being read", () => {
  it("shows no other hook's script and cannot be saved yet", async () => {
    const ipc = await import("$lib/ipc");
    vi.mocked(ipc.writeHook).mockClear();
    hooks.overview = {
      hooks: [
        { name: "pre-commit", state: "enabled" },
        { name: "commit-msg", state: "enabled" },
      ],
    } as never;
    const first = hooks.edit(1 as never, "pre-commit");
    reads.get("pre-commit")?.("commit script");
    await first;

    const second = hooks.edit(1 as never, "commit-msg");
    expect(hooks.body).toBe("");
    await hooks.save(1 as never);
    expect(ipc.writeHook).not.toHaveBeenCalled();

    reads.get("commit-msg")?.("message script");
    await second;
    expect(hooks.body).toBe("message script");
  });
});
