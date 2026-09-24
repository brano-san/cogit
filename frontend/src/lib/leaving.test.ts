import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/ipc", () => ({ CogitError: class extends Error {} }));

const { confirmation } = await import("$stores/confirm.svelte");
const { prompt } = await import("$stores/prompt.svelte");
const { stashDialog } = await import("$stores/stash-dialog.svelte");
const { hooks } = await import("$stores/hooks.svelte");
const { leaveRepositoryDialogs } = await import("./leaving");

// Every one of these was asked about the repository on screen; once the panels show
// another, answering it would act there.
describe("leaving a repository", () => {
  it("answers its open questions as cancelled", async () => {
    const asked = confirmation.ask({ title: "Discard", message: "a.txt", confirm: "Discard" });
    leaveRepositoryDialogs();
    expect(await asked).toBe(false);

    const typed = prompt.ask({ title: "Rename", label: "Name", confirm: "Rename" });
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
});
