import { describe, expect, it, vi } from "vitest";
import type { RepoId } from "$lib/ipc";

const commands = { commitSignatures: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands, events: {} }));

const { signatures } = await import("./signatures.svelte");

const repo = 1 as unknown as RepoId;

describe("signatures.load", () => {
  it("asks for an oid once, even when it came back without a status", async () => {
    commands.commitSignatures.mockResolvedValue({
      status: "ok",
      data: [{ oid: "a", status: "G", signer: "me" }],
    });

    await signatures.load(repo, ["a", "gone"]);
    await signatures.load(repo, ["a", "gone"]);

    expect(commands.commitSignatures).toHaveBeenCalledTimes(1);
    expect(signatures.statuses.get("a")?.status).toBe("G");
  });

  it("does not ask again after a failure", async () => {
    commands.commitSignatures.mockReset();
    commands.commitSignatures.mockResolvedValue({ status: "error", error: { kind: "io", data: "gpg" } });

    await signatures.load(repo, ["b"]);
    await signatures.load(repo, ["b"]);

    expect(commands.commitSignatures).toHaveBeenCalledTimes(1);
  });
});
