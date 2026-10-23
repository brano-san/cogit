import { beforeEach, describe, expect, it, vi } from "vitest";

type Known = { host: string; username: string | null; keyPath: string | null };

const backend = vi.hoisted(() => ({
  known: [] as Known[],
  state: "unlocked" as "missing" | "locked" | "unlocked",
}));

vi.mock("$lib/ipc", () => ({
  credentialStore: vi.fn(async () => ({
    state: backend.state,
    masterPassword: true,
    known: backend.state === "unlocked" ? backend.known : [],
  })),
  storeCredential: vi.fn(async (credential: { host: string; username: string | null; keyPath?: string | null }) => {
    backend.known = [...backend.known, { host: credential.host, username: credential.username, keyPath: credential.keyPath ?? null }];
  }),
  forgetCredential: vi.fn(async (host: string, ssh: boolean) => {
    backend.known = backend.known.filter((entry) => entry.host !== host || (entry.keyPath !== null) !== ssh);
  }),
  forgetAllCredentials: vi.fn(async () => {
    backend.known = [];
  }),
}));

const report = vi.hoisted(() => vi.fn());
vi.mock("$stores/notices.svelte", () => ({ notices: { report } }));
const ask = vi.hoisted(() => vi.fn(async () => true));
vi.mock("$stores/confirm.svelte", () => ({ confirmation: { ask } }));
const typed = vi.hoisted(() => ({ answer: null as unknown }));
vi.mock("$stores/sign-in.svelte", () => ({ signIn: { ask: vi.fn(async () => typed.answer) } }));
const ready = vi.hoisted(() => ({ value: true }));
vi.mock("$stores/master-password.svelte", () => ({
  masterPassword: { ready: vi.fn(async () => ready.value), unlock: vi.fn(), change: vi.fn() },
}));

const ipc = await import("$lib/ipc");
const { credentials } = await import("./credentials.svelte");

const ann: Known = { host: "a.example", username: "ann", keyPath: null };

beforeEach(() => {
  backend.known = [ann];
  backend.state = "unlocked";
  ready.value = true;
  vi.clearAllMocks();
});

describe("Preferences ▸ Authentication", () => {
  it("lists what the backend holds, and nothing while it is locked", async () => {
    await credentials.refresh();
    expect(credentials.known).toEqual([ann]);

    backend.state = "locked";
    await credentials.refresh();
    expect(credentials.known).toEqual([]);
    expect(credentials.state).toBe("locked");
  });

  it("adds what the dialog was given, and nothing when it or the store was canceled", async () => {
    typed.answer = null;
    await credentials.add();
    expect(ipc.storeCredential).not.toHaveBeenCalled();

    typed.answer = { host: "b.example", username: null, secret: "tok", keyPath: null, remember: true };
    ready.value = false;
    await credentials.add();
    expect(ipc.storeCredential).not.toHaveBeenCalled();

    ready.value = true;
    await credentials.add();
    expect(ipc.storeCredential).toHaveBeenCalledWith(typed.answer);
    expect(credentials.known.map((entry) => entry.host)).toEqual(["a.example", "b.example"]);
  });

  it("removes a login and an ssh key of one host separately, and all only after a yes", async () => {
    const key: Known = { host: "a.example", username: null, keyPath: "/k/id" };
    backend.known = [ann, key];
    await credentials.remove(key);
    expect(ipc.forgetCredential).toHaveBeenCalledWith("a.example", true);
    expect(credentials.known).toEqual([ann]);

    ask.mockResolvedValueOnce(false);
    await credentials.removeAll();
    expect(ipc.forgetAllCredentials).not.toHaveBeenCalled();

    await credentials.removeAll();
    expect(ipc.forgetAllCredentials).toHaveBeenCalled();
    expect(credentials.known).toEqual([]);
  });

  it("reports a store that refuses and keeps showing what is stored", async () => {
    const refused = new Error("locked");
    vi.mocked(ipc.forgetCredential).mockRejectedValueOnce(refused);
    await credentials.refresh();

    await credentials.remove(ann);

    expect(report).toHaveBeenCalledWith(refused, "Could not remove the sign-in");
    expect(credentials.known).toEqual([ann]);
  });
});
