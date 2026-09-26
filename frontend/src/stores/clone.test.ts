import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  clipboard: null as string | null,
  listing: { defaultBranch: "main", branches: ["dev", "main"] } as {
    defaultBranch: string | null;
    branches: string[];
  },
  failure: null as unknown,
  kinds: new Map<string, "missing" | "empty" | "notEmpty">(),
}));

vi.mock("$lib/ipc/clone", () => ({
  clipboardRepositoryUrl: vi.fn(async () => backend.clipboard),
  remoteBranches: vi.fn(async () => {
    if (backend.failure) throw backend.failure;
    return backend.listing;
  }),
  cloneDestination: vi.fn(async (path: string) => backend.kinds.get(path) ?? "missing"),
}));

vi.mock("$lib/ipc", () => ({
  toCogitError: (err: unknown) => err,
}));

const ipc = await import("$lib/ipc/clone");
const { CloneWizard } = await import("./clone.svelte");

beforeEach(() => {
  backend.clipboard = null;
  backend.failure = null;
  backend.kinds.clear();
  vi.clearAllMocks();
});

async function settled() {
  for (let i = 0; i < 5; i += 1) await Promise.resolve();
}

describe("opening the wizard", () => {
  it("takes a repository URL from the clipboard and names the folder after it", async () => {
    backend.clipboard = "https://github.com/owner/app.git";
    const wizard = new CloneWizard();

    await wizard.start("D:\\src");

    expect(wizard.source).toBe("https://github.com/owner/app.git");
    expect(wizard.name).toBe("app");
    expect(wizard.dirty).toBe(false);
  });

  it("does not overwrite what was typed before the clipboard answered", async () => {
    backend.clipboard = "https://github.com/owner/app.git";
    const wizard = new CloneWizard();
    const starting = wizard.start("D:\\src");
    wizard.setSource("D:\\repos\\mine");
    await starting;

    expect(wizard.source).toBe("D:\\repos\\mine");
    expect(wizard.dirty).toBe(true);
  });
});

describe("the first page", () => {
  it("checks access on Next, then shows the server's branches", async () => {
    const wizard = new CloneWizard();
    await wizard.start("D:\\src");
    wizard.setSource("https://host/app.git");

    await wizard.next();

    expect(ipc.remoteBranches).toHaveBeenCalledWith("https://host/app.git");
    expect(wizard.page).toBe("selection");
    expect(wizard.branch).toBe("main");
    expect(wizard.branches[0]).toEqual(["main", "main (default)"]);
    expect(wizard.dirty).toBe(true);
  });

  it("stays on a failed check and offers to go on without it", async () => {
    backend.failure = { message: "failed", detail: { kind: "command" } };
    const wizard = new CloneWizard();
    await wizard.start("D:\\src");
    wizard.setSource("https://host/private.git");

    await wizard.next();

    expect(wizard.page).toBe("repository");
    expect(wizard.failed).toBe(backend.failure);
    wizard.continueUnchecked();
    expect(wizard.page).toBe("selection");
    expect(wizard.branchReason).toBe("The branches are listed once the check succeeds");
  });

  it("forgets a failure once the URL changes", async () => {
    backend.failure = { message: "failed", detail: { kind: "command" } };
    const wizard = new CloneWizard();
    await wizard.start("D:\\src");
    wizard.setSource("https://host/typo.git");
    await wizard.next();

    wizard.setSource("https://host/app.git");

    expect(wizard.failed).toBeNull();
  });

  it("keeps Next inactive while the check runs", async () => {
    const wizard = new CloneWizard();
    await wizard.start("D:\\src");
    wizard.setSource("https://host/app.git");

    const checking = wizard.next();

    expect(wizard.problem).toBe("Checking access to the repository…");
    await checking;
  });

  it("drops the answer of a check the wizard was closed on", async () => {
    const wizard = new CloneWizard();
    await wizard.start("D:\\src");
    wizard.setSource("https://host/app.git");
    const checking = wizard.next();
    wizard.close();
    await checking;

    expect(wizard.page).toBe("repository");
  });
});

describe("the last page", () => {
  async function onDirectory(wizard: InstanceType<typeof CloneWizard>) {
    await wizard.start("D:\\src");
    wizard.setSource("https://host/app.git");
    await wizard.next();
    await wizard.next();
    await settled();
  }

  it("finishes into a missing folder", async () => {
    const wizard = new CloneWizard();
    await onDirectory(wizard);

    expect(wizard.page).toBe("directory");
    expect(wizard.problem).toBeNull();
    expect(wizard.finish()).toMatchObject({ source: "https://host/app.git", target: "D:\\src\\app" });
  });

  it("keeps Finish inactive on a folder that is not empty, and says so", async () => {
    backend.kinds.set("D:\\src\\taken", "notEmpty");
    const wizard = new CloneWizard();
    await onDirectory(wizard);

    wizard.setName("taken");
    await settled();

    expect(wizard.problem).toBe("D:\\src\\taken is not empty");
    expect(wizard.finish()).toBeNull();
  });

  it("keeps a name the user typed when going back to change the URL", async () => {
    const wizard = new CloneWizard();
    await onDirectory(wizard);
    wizard.setName("mine");

    wizard.back();
    wizard.back();
    wizard.setSource("https://host/other.git");

    expect(wizard.name).toBe("mine");
  });
});
