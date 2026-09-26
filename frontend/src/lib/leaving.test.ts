import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/ipc", () => ({ CogitError: class extends Error {} }));

const { confirmation } = await import("$stores/confirm.svelte");
const { prompt } = await import("$stores/prompt.svelte");
const { stashDialog } = await import("$stores/stash-dialog.svelte");
const { hooks } = await import("$stores/hooks.svelte");
const { remoteOps } = await import("$stores/remote-ops.svelte");
const { refDialogs } = await import("$stores/ref-dialogs.svelte");
const { remoteDialogs } = await import("$stores/remote-dialogs.svelte");
const { leaveRepositoryDialogs } = await import("./leaving");

// Every one of these was asked about the repository on screen; once the panels show
// another, answering it would act there.
describe("leaving a repository", () => {
  it("answers its open questions as cancelled", async () => {
    const asked = confirmation.ask({ title: "Discard", message: "a.txt", confirm: "Discard" });
    leaveRepositoryDialogs();
    expect(await asked).toBe(false);

    const typed = prompt.ask({ title: "Rename", label: "Name", confirm: "Rename", validate: () => null });
    leaveRepositoryDialogs();
    expect(await typed).toBeNull();

    const stashing = stashDialog.create();
    leaveRepositoryDialogs();
    expect(await stashing).toBeNull();
  });

  it("closes the hook editor", () => {
    hooks.open = true;
    hooks.editing = "pre-commit";
    hooks.body = "#!/bin/sh\n";

    leaveRepositoryDialogs();

    expect(hooks.open).toBe(false);
    expect(hooks.editing).toBeNull();
  });

  // Reset Advanced… left open over B reset B's branch to A's commit on submit; Push To…
  // pushed B's branch of the same name; a remote operation ran in A while B was on screen.
  it("closes the dialogs of the reference menus and of the remote operations", () => {
    refDialogs.tag = { oid: "a1", subject: "fix" };
    refDialogs.push = { kind: "branch", name: "main", upstream: "origin/main" };
    refDialogs.reset = { oid: "a1", subject: "fix", moving: "main" };
    refDialogs.message = { oid: "a1", message: "fix", parents: [] };
    refDialogs.author = { oid: "a1", name: "Ann", email: "ann@example.com" };
    const spec = { title: "Add Submodule", intro: "", fields: [], confirm: "Add", destructive: false, values: {} };
    remoteOps.dialog = { spec, submit: async () => {} };

    leaveRepositoryDialogs();

    expect(refDialogs.tag).toBeNull();
    expect(refDialogs.push).toBeNull();
    expect(refDialogs.reset).toBeNull();
    expect(refDialogs.message).toBeNull();
    expect(refDialogs.author).toBeNull();
    expect(remoteOps.dialog).toBeNull();
  });

  // Properties of B's origin saved A's URL into B.
  it("closes the Properties of a remote", () => {
    remoteDialogs.properties = { name: "origin", url: "x", pushUrl: null, backgroundFetch: true, shallow: false };

    leaveRepositoryDialogs();

    expect(remoteDialogs.properties).toBeNull();
  });
});
