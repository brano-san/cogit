import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = {
  repositoryHealth: vi.fn(),
  readSetting: vi.fn(),
  writeSetting: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));
vi.mock("$lib/settings-file", () => ({ readKey: vi.fn(async () => null), writeKey: vi.fn(async () => {}) }));

const { health } = await import("./health.svelte");

const finding = {
  module: "",
  issue: { kind: "ignoreCaseMismatch", configured: false, actual: true },
};

// With R-351 a click on another repository and back is a switch, not an open: "Remind me
// later" lasted until the first click elsewhere and the warning came back on every return.
describe("Remind me later", () => {
  beforeEach(() => {
    commands.repositoryHealth.mockReset();
    commands.repositoryHealth.mockResolvedValue({ status: "ok", data: [finding] });
  });

  it("outlasts a switch to another repository and back", async () => {
    await health.check(1, "C:/repos/a", "a");
    const [warning] = health.warnings;
    health.remindLater(warning!.id);

    await health.check(2, "C:/repos/b", "b");
    expect(health.warnings).toHaveLength(1);
    await health.check(1, "C:/repos/a", "a");

    expect(health.warnings).toEqual([]);
  });

  it("lasts until the repository opens again, closed in between", async () => {
    await health.check(5, "C:/repos/c", "c");
    health.remindLater(health.warnings[0]!.id);

    // A repository closed and opened again comes back under a new id.
    await health.check(6, "C:/repos/c", "c");

    expect(health.warnings).toHaveLength(1);
  });
});

it("does not restore a commit-graph warning from a scan that began before the write", async () => {
  const missing = { module: "vendor/lib", issue: { kind: "noCommitGraph" } };
  commands.repositoryHealth.mockReset();
  commands.repositoryHealth.mockResolvedValueOnce({ status: "ok", data: [missing] });
  await health.check(10, "C:/repos/nested", "nested");
  expect(health.warnings[0]?.action?.targets).toEqual(["vendor/lib"]);

  let answerOld: (value: unknown) => void = () => {};
  commands.repositoryHealth.mockImplementationOnce(() => new Promise((resolve) => (answerOld = resolve)));
  const old = health.recheck();
  commands.repositoryHealth.mockResolvedValueOnce({ status: "ok", data: [] });
  await health.recheck();
  answerOld({ status: "ok", data: [missing] });
  await old;

  expect(health.warnings).toEqual([]);
});
