import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  known: [] as { host: string; username: string | null }[],
  persistent: true,
}));

vi.mock("$lib/ipc", () => ({
  listCredentials: vi.fn(async () => ({ persistent: backend.persistent, known: backend.known })),
  storeCredential: vi.fn(async (credential: { host: string; username: string | null }) => {
    backend.known = [...backend.known, { host: credential.host, username: credential.username }];
  }),
  forgetCredential: vi.fn(async (host: string) => {
    backend.known = backend.known.filter((entry) => entry.host !== host);
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

const ipc = await import("$lib/ipc");
const { credentials } = await import("./credentials.svelte");

beforeEach(() => {
  backend.known = [{ host: "a.example", username: "ann" }];
  vi.clearAllMocks();
});

describe("Preferences ▸ Authentication", () => {
  it("lists what the backend holds", async () => {
    await credentials.refresh();
    expect(credentials.known).toEqual([{ host: "a.example", username: "ann" }]);
  });

  it("adds what the dialog was given, and nothing when it was canceled", async () => {
    typed.answer = null;
    await credentials.add();
    expect(ipc.storeCredential).not.toHaveBeenCalled();

    typed.answer = { host: "b.example", username: null, secret: "tok", remember: true };
    await credentials.add();
    expect(ipc.storeCredential).toHaveBeenCalledWith(typed.answer);
    expect(credentials.known.map((entry) => entry.host)).toEqual(["a.example", "b.example"]);
  });

  it("removes one host, and all of them only after a yes", async () => {
    await credentials.remove("a.example");
    expect(credentials.known).toEqual([]);

    backend.known = [{ host: "a.example", username: null }];
    ask.mockResolvedValueOnce(false);
    await credentials.removeAll();
    expect(ipc.forgetAllCredentials).not.toHaveBeenCalled();

    await credentials.removeAll();
    expect(ipc.forgetAllCredentials).toHaveBeenCalled();
    expect(credentials.known).toEqual([]);
  });

  it("reports a keychain that refuses and keeps showing what is stored", async () => {
    const refused = new Error("locked");
    vi.mocked(ipc.forgetCredential).mockRejectedValueOnce(refused);
    await credentials.refresh();

    await credentials.remove("a.example");

    expect(report).toHaveBeenCalledWith(refused, "Could not remove the sign-in");
    expect(credentials.known).toEqual([{ host: "a.example", username: "ann" }]);
  });
});
