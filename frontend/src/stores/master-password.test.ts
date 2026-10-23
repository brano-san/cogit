import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  state: "locked" as "missing" | "locked" | "unlocked",
  password: "master" as string | null,
}));

vi.mock("$lib/ipc", () => ({
  credentialStore: vi.fn(async () => ({ state: backend.state, masterPassword: backend.password !== null, known: [] })),
  unlockCredentials: vi.fn(async (password: string) => {
    const right = password === backend.password;
    if (right) backend.state = "unlocked";
    return right;
  }),
  createCredentialStore: vi.fn(async (password: string | null) => {
    backend.state = "unlocked";
    backend.password = password;
  }),
  changeMasterPassword: vi.fn(async (current: string, next: string | null) => {
    if (current !== (backend.password ?? "")) return false;
    backend.password = next;
    return true;
  }),
  resetCredentialStore: vi.fn(async (password: string | null) => {
    backend.state = "unlocked";
    backend.password = password;
  }),
}));

const ipc = await import("$lib/ipc");
const { masterPassword } = await import("./master-password.svelte");

/** Answers each dialog in turn, recording how it was opened. */
async function answering(answers: unknown[], run: () => Promise<unknown>) {
  const seen: { mode: string; error: string | null }[] = [];
  const done = run();
  for (const answer of answers) {
    await vi.waitFor(() => expect(masterPassword.open).not.toBeNull());
    const open = masterPassword.open!;
    seen.push({ mode: open.mode, error: open.error });
    masterPassword.answer(answer as never);
  }
  return { result: await done, seen };
}

beforeEach(() => {
  backend.state = "locked";
  backend.password = "master";
  vi.clearAllMocks();
});

describe("the master password", () => {
  it("is asked again after a wrong one, then not before the next command", async () => {
    const { result, seen } = await answering(
      [{ current: "wrong", password: null }, { current: "master", password: null }],
      () => masterPassword.unlock(),
    );
    expect(result).toBe(true);
    expect(seen).toEqual([
      { mode: "unlock", error: null },
      { mode: "unlock", error: "Wrong master password." },
    ]);

    await masterPassword.beforeUse();
    expect(masterPassword.open).toBeNull();
  });

  it("canceled, it is not asked before every command, but is when a sign-in is kept", async () => {
    const { result } = await answering([null], () => masterPassword.beforeUse());
    expect(result).toBeUndefined();
    await masterPassword.beforeUse();
    expect(masterPassword.open).toBeNull();

    const kept = await answering([{ current: "master", password: null }], () => masterPassword.ready());
    expect(kept.result).toBe(true);
  });

  it("forgotten, sets a new one and wipes the store", async () => {
    const { result, seen } = await answering(["forgot", { current: "", password: "new" }], () => masterPassword.unlock());
    expect(result).toBe(true);
    expect(seen.map((s) => s.mode)).toEqual(["unlock", "forgot"]);
    expect(ipc.resetCredentialStore).toHaveBeenCalledWith("new");
  });

  it("is set, or declined, when the first sign-in is kept", async () => {
    backend.state = "missing";
    backend.password = null;
    const { seen } = await answering([{ current: "", password: null }], () => masterPassword.ready());
    expect(seen[0]!.mode).toBe("create");
    expect(ipc.createCredentialStore).toHaveBeenCalledWith(null);
  });

  it("changes only with the current one", async () => {
    backend.state = "unlocked";
    const { result, seen } = await answering(
      [{ current: "nope", password: "b" }, { current: "master", password: "b" }],
      () => masterPassword.change(),
    );
    expect(result).toBe(true);
    expect(seen[1]!.error).toBe("The current master password is wrong.");
    expect(backend.password).toBe("b");
  });
});
